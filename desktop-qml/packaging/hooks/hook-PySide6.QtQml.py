"""Collect only the Qt QML modules used by this client.

The default PyInstaller QtQml hook bundles every module installed on the build
host. That can pull in unrelated KDE, WebEngine, 3D and chart stacks. Keep the
normal QtQml dependencies, then retain the Qt QML roots used by our QML files.
"""
from pathlib import PurePosixPath

from PyInstaller.utils.hooks.qt import add_qt6_dependencies, pyside6_library_info

hiddenimports, binaries, datas = add_qt6_dependencies(__file__)
all_qml_binaries, all_qml_datas = pyside6_library_info.collect_qtqml_files()

_QML_ROOTS = {"Qt", "QtCore", "QtQml", "QtQuick", "QtMultimedia"}
_QUICK_SUBTREES = {"Controls", "Dialogs", "Effects", "Layouts", "Templates", "Window"}


def _is_used_qml_root(destination: str) -> bool:
    parts = PurePosixPath(destination).parts
    try:
        qml_index = parts.index("qml")
        root = parts[qml_index + 1]
    except (ValueError, IndexError):
        return False
    if root not in _QML_ROOTS:
        return False
    if root != "QtQuick":
        return True
    subtree = parts[qml_index + 2] if len(parts) > qml_index + 2 else ""
    return not subtree or subtree in _QUICK_SUBTREES


binaries += [item for item in all_qml_binaries if _is_used_qml_root(item[1])]
datas += [item for item in all_qml_datas if _is_used_qml_root(item[1])]
