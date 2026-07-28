import os
import shutil

defines = globals().get("defines", {})

app_path = os.path.abspath(defines["app"])
app_name = os.path.basename(app_path)

files = [app_path]
symlinks = {"Applications": "/Applications"}
icon = os.path.abspath(defines["icon"])

format = "UDZO"
filesystem = "HFS+"
background = os.path.abspath(defines["background"])
window_rect = ((100, 100), (640, 420))

default_view = "icon-view"
include_icon_view_settings = True
arrange_by = None
icon_size = 128
text_size = 14
icon_locations = {
    app_name: (160, 220),
    "Applications": (480, 220),
    ".background.tiff": (800, 600),
    ".VolumeIcon.icns": (900, 600),
}
hide = [".background.tiff", ".VolumeIcon.icns"]
hide_extensions = [app_name]


def create_hook(mount_point, options):
    shutil.rmtree(os.path.join(mount_point, ".fseventsd"), ignore_errors=True)
