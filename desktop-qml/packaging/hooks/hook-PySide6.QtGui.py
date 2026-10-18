"""Keep image loaders used by chat media and icon assets; omit unrelated codecs."""
from pathlib import PurePosixPath

from PyInstaller.utils.hooks.qt import add_qt6_dependencies

hiddenimports, binaries, datas = add_qt6_dependencies(__file__)

_IMAGE_FORMATS = {"libqgif.so", "libqico.so", "libqjpeg.so", "libqsvg.so", "libqwebp.so"}


def _keep(item: tuple) -> bool:
    source = PurePosixPath(item[0])
    if "imageformats" not in source.parts:
        return True
    return source.name in _IMAGE_FORMATS


binaries = [item for item in binaries if _keep(item)]
datas = [item for item in datas if _keep(item)]
