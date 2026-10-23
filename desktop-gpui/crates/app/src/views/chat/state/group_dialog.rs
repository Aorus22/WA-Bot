

#[derive(Clone)]
pub struct NewGroupDialog {
    pub is_open: bool,
    pub subject: String,
    pub participant_input: String,
}

impl NewGroupDialog {
    pub fn new() -> Self {
        Self {
            is_open: false,
            subject: String::new(),
            participant_input: String::new(),
        }
    }

    pub fn open(&mut self) {
        self.is_open = true;
        self.subject.clear();
        self.participant_input.clear();
    }

    pub fn close(&mut self) {
        self.is_open = false;
    }
}

#[derive(Clone)]
pub struct JoinGroupDialog {
    pub is_open: bool,
    pub link: String,
    pub preview_name: Option<String>,
    pub preview_count: Option<u32>,
}

impl JoinGroupDialog {
    pub fn new() -> Self {
        Self {
            is_open: false,
            link: String::new(),
            preview_name: None,
            preview_count: None,
        }
    }

    pub fn open(&mut self) {
        self.is_open = true;
        self.link.clear();
        self.preview_name = None;
        self.preview_count = None;
    }

    pub fn close(&mut self) {
        self.is_open = false;
    }
}
