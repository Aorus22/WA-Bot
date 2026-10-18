# Third-party runtime notices

The QML runtime depends on PySide6, PyGObject, GStreamer, the GStreamer OpenH264
plugin, segno, and dbus-next. Review the notices for the exact versions shipped
by the target Linux distribution or included by a future bundle.

## Call video: GStreamer and OpenH264

Call video uses GStreamer's `appsrc`/`appsink` APIs and the system-provided
`openh264enc`, `openh264dec`, and `h264parse` elements. The application bundle
does not copy those plugin binaries; the host must have GStreamer with OpenH264
support installed. The tested OpenH264 GStreamer plugin reports BSD licensing.
OpenH264 itself is BSD-licensed, and Cisco's binary distribution also carries
terms for its AVC/H.264 patent portfolio. Follow the notices and terms supplied
by the host distribution's GStreamer/OpenH264 packages.

References:

- [GStreamer OpenH264 plugin](https://gstreamer.freedesktop.org/documentation/openh264/openh264enc.html)
- [GStreamer appsrc](https://gstreamer.freedesktop.org/documentation/app/appsrc.html)
- [OpenH264 FAQ and binary terms](https://www.openh264.org/faq.html)
