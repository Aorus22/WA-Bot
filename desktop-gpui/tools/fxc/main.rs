// Standalone FXC compiler replacement using d3dcompiler_47.dll
// Avoids requiring the multi-gigabyte Windows 10/11 SDK just for fxc.exe.

use std::env;
use std::ffi::{c_char, c_void, CString};
use std::fs;
use std::path::PathBuf;
use std::process::exit;

#[repr(C)]
struct ID3DBlobVtbl {
    query_interface: unsafe extern "system" fn(this: *mut ID3DBlob, riid: *const c_void, ppv: *mut *mut c_void) -> i32,
    add_ref: unsafe extern "system" fn(this: *mut ID3DBlob) -> u32,
    release: unsafe extern "system" fn(this: *mut ID3DBlob) -> u32,
    get_buffer_pointer: unsafe extern "system" fn(this: *mut ID3DBlob) -> *const u8,
    get_buffer_size: unsafe extern "system" fn(this: *mut ID3DBlob) -> usize,
}

#[repr(C)]
struct ID3DBlob {
    vtbl: *const ID3DBlobVtbl,
}

impl ID3DBlob {
    unsafe fn data(&self) -> &[u8] {
        let ptr = ((*self.vtbl).get_buffer_pointer)(self as *const _ as *mut _);
        let len = ((*self.vtbl).get_buffer_size)(self as *const _ as *mut _);
        std::slice::from_raw_parts(ptr, len)
    }

    unsafe fn release(&mut self) {
        ((*self.vtbl).release)(self as *mut _);
    }
}

type D3DCompileFn = unsafe extern "system" fn(
    p_src_data: *const u8,
    src_data_size: usize,
    p_source_name: *const c_char,
    p_defines: *const c_void,
    p_include: *const c_void,
    p_entrypoint: *const c_char,
    p_target: *const c_char,
    flags1: u32,
    flags2: u32,
    pp_code: *mut *mut ID3DBlob,
    pp_error_msgs: *mut *mut ID3DBlob,
) -> i32;

extern "system" {
    fn LoadLibraryA(lp_lib_filename: *const c_char) -> *mut c_void;
    fn GetProcAddress(h_module: *mut c_void, lp_proc_name: *const c_char) -> *const c_void;
    fn FreeLibrary(h_module: *mut c_void) -> i32;
}

fn print_usage() {
    eprintln!("Usage: fxc [/T target] [/E entrypoint] [/Fh header_file] [/Vn var_name] [/Fo obj_file] <source_file>");
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        print_usage();
        exit(1);
    }

    let mut target = String::new();
    let mut entrypoint = String::new();
    let mut header_output: Option<PathBuf> = None;
    let mut object_output: Option<PathBuf> = None;
    let mut var_name = String::from("g_shader");
    let mut flags1: u32 = 0;
    let flags2: u32 = 0;
    let mut source_file: Option<PathBuf> = None;

    let mut i = 1;
    while i < args.len() {
        let arg = &args[i];
        if arg.starts_with('/') || arg.starts_with('-') {
            let flag = &arg[1..];
            if flag.eq_ignore_ascii_case("T") {
                i += 1;
                if i < args.len() { target = args[i].clone(); }
            } else if flag.to_ascii_uppercase().starts_with("T") && flag.len() > 1 {
                target = flag[1..].to_string();
            } else if flag.eq_ignore_ascii_case("E") {
                i += 1;
                if i < args.len() { entrypoint = args[i].clone(); }
            } else if flag.to_ascii_uppercase().starts_with("E") && flag.len() > 1 {
                entrypoint = flag[1..].to_string();
            } else if flag.eq_ignore_ascii_case("Fh") {
                i += 1;
                if i < args.len() { header_output = Some(PathBuf::from(&args[i])); }
            } else if flag.to_ascii_uppercase().starts_with("FH") && flag.len() > 2 {
                header_output = Some(PathBuf::from(&flag[2..]));
            } else if flag.eq_ignore_ascii_case("Fo") {
                i += 1;
                if i < args.len() { object_output = Some(PathBuf::from(&args[i])); }
            } else if flag.to_ascii_uppercase().starts_with("FO") && flag.len() > 2 {
                object_output = Some(PathBuf::from(&flag[2..]));
            } else if flag.eq_ignore_ascii_case("Vn") {
                i += 1;
                if i < args.len() { var_name = args[i].clone(); }
            } else if flag.to_ascii_uppercase().starts_with("VN") && flag.len() > 2 {
                var_name = flag[2..].to_string();
            } else if flag.eq_ignore_ascii_case("O3") {
                // D3DCOMPILE_OPTIMIZATION_LEVEL3 = (1 << 15)
                flags1 |= 1 << 15;
            } else if flag.eq_ignore_ascii_case("O2") {
                flags1 |= (1 << 14) | (1 << 15);
            } else if flag.eq_ignore_ascii_case("O1") {
                flags1 |= 0;
            } else if flag.eq_ignore_ascii_case("O0") {
                flags1 |= 1 << 14;
            } else if flag.eq_ignore_ascii_case("nologo") {
                // Ignore nologo
            } else {
                // Unknown flag, ignore for compatibility
            }
        } else {
            source_file = Some(PathBuf::from(arg));
        }
        i += 1;
    }

    let source_path = match source_file {
        Some(p) => p,
        None => {
            eprintln!("Error: No source file specified.");
            exit(1);
        }
    };

    if target.is_empty() {
        eprintln!("Error: Target profile (/T) must be specified.");
        exit(1);
    }
    if entrypoint.is_empty() {
        eprintln!("Error: Entry point (/E) must be specified.");
        exit(1);
    }

    let full_source_path = match fs::canonicalize(&source_path) {
        Ok(p) => p,
        Err(_) => source_path.clone(),
    };

    let source_code = match fs::read(&full_source_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error: Failed to read shader source file {:?}: {}", full_source_path, e);
            exit(1);
        }
    };

    // Load d3dcompiler_47.dll
    let dll_names = ["d3dcompiler_47.dll", "d3dcompiler_46.dll", "d3dcompiler_43.dll"];
    let mut h_module: *mut c_void = std::ptr::null_mut();
    for name in &dll_names {
        let c_name = CString::new(*name).unwrap();
        unsafe {
            h_module = LoadLibraryA(c_name.as_ptr());
            if !h_module.is_null() {
                break;
            }
        }
    }

    if h_module.is_null() {
        eprintln!("Error: Could not load d3dcompiler_47.dll (or fallback versions).");
        exit(1);
    }

    let proc_name = CString::new("D3DCompile").unwrap();
    let d3d_compile_ptr = unsafe { GetProcAddress(h_module, proc_name.as_ptr()) };
    if d3d_compile_ptr.is_null() {
        eprintln!("Error: D3DCompile symbol not found in compiler DLL.");
        unsafe { FreeLibrary(h_module); }
        exit(1);
    }

    let d3d_compile: D3DCompileFn = unsafe { std::mem::transmute(d3d_compile_ptr) };

    let source_name_str = full_source_path.to_string_lossy().to_string();
    let c_source_name = CString::new(source_name_str.as_bytes()).unwrap_or_else(|_| CString::new("shader.hlsl").unwrap());
    let c_entrypoint = CString::new(entrypoint.as_str()).unwrap();
    let c_target = CString::new(target.as_str()).unwrap();

    let mut code_blob: *mut ID3DBlob = std::ptr::null_mut();
    let mut error_blob: *mut ID3DBlob = std::ptr::null_mut();

    // D3D_COMPILE_STANDARD_FILE_INCLUDE = ((ID3DInclude*)(UINT_PTR)1)
    let standard_include = 1usize as *const c_void;

    let hr = unsafe {
        d3d_compile(
            source_code.as_ptr(),
            source_code.len(),
            c_source_name.as_ptr(),
            std::ptr::null(),
            standard_include,
            c_entrypoint.as_ptr(),
            c_target.as_ptr(),
            flags1,
            flags2,
            &mut code_blob,
            &mut error_blob,
        )
    };

    if hr < 0 {
        if !error_blob.is_null() {
            unsafe {
                let err_data = (*error_blob).data();
                let err_str = String::from_utf8_lossy(err_data);
                eprintln!("{}", err_str);
                (*error_blob).release();
            }
        } else {
            eprintln!("Shader compilation failed with HRESULT: 0x{:08X}", hr as u32);
        }
        if !code_blob.is_null() {
            unsafe { (*code_blob).release(); }
        }
        unsafe { FreeLibrary(h_module); }
        exit(1);
    }

    if !error_blob.is_null() {
        unsafe {
            // Warnings may be present even on success
            let err_data = (*error_blob).data();
            let err_str = String::from_utf8_lossy(err_data);
            if !err_str.trim().is_empty() {
                eprintln!("{}", err_str);
            }
            (*error_blob).release();
        }
    }

    if code_blob.is_null() {
        eprintln!("Error: D3DCompile succeeded but returned no code blob.");
        unsafe { FreeLibrary(h_module); }
        exit(1);
    }

    let bytecode = unsafe { (*code_blob).data() };

    // If object output path was specified, write raw bytecode
    if let Some(ref obj_path) = object_output {
        if let Some(parent) = obj_path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Err(e) = fs::write(obj_path, bytecode) {
            eprintln!("Error: Failed to write object file {:?}: {}", obj_path, e);
            unsafe {
                (*code_blob).release();
                FreeLibrary(h_module);
            }
            exit(1);
        }
    }

    // If header output path was specified, write C header array
    if let Some(ref hdr_path) = header_output {
        if let Some(parent) = hdr_path.parent() {
            let _ = fs::create_dir_all(parent);
        }

        let mut header_content = String::new();
        header_content.push_str(&format!("const BYTE {}[] = {{\n", var_name));
        for (idx, byte) in bytecode.iter().enumerate() {
            if idx % 16 == 0 {
                header_content.push_str("    ");
            }
            header_content.push_str(&format!("{:3}", byte));
            if idx + 1 < bytecode.len() {
                header_content.push(',');
            }
            if (idx + 1) % 16 == 0 || idx + 1 == bytecode.len() {
                header_content.push('\n');
            } else {
                header_content.push(' ');
            }
        }
        header_content.push_str("};\n");

        if let Err(e) = fs::write(hdr_path, header_content) {
            eprintln!("Error: Failed to write header file {:?}: {}", hdr_path, e);
            unsafe {
                (*code_blob).release();
                FreeLibrary(h_module);
            }
            exit(1);
        }
    }

    unsafe {
        (*code_blob).release();
        FreeLibrary(h_module);
    }
}
