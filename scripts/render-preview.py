"""Headless model preview for `make previews`, run via Blender.

    blender --background --python-exit-code 1 --python scripts/render-preview.py -- \\
        <model.glb> <out.png> [azimuth_deg] [elevation_deg]

APPLIES the pose from scripts/preview_plan.py (the pure, tested framing
rule) to the imported model and renders one Workbench frame — studio
lighting, textured color, a neutral backdrop — into out/metrics/ beside
the model's metrics TOML. A reading aid for identification and review
(a seller's listing matched by eye, a hull checked before it reaches
the roster), never a gate. Computes nothing of its own beyond the
model's Blender-space bounds.
"""
import math
import os
import sys

import bpy
from mathutils import Vector

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from preview_plan import plan_preview  # noqa: E402

WIDTH, HEIGHT = 1024, 768
FOV_DEG = 40.0
BACKDROP = (0.16, 0.17, 0.20)

argv = sys.argv[sys.argv.index("--") + 1:]
in_path, out_path = argv[0], argv[1]
azimuth = float(argv[2]) if len(argv) > 2 else 35.0
elevation = float(argv[3]) if len(argv) > 3 else 25.0

bpy.ops.wm.read_factory_settings(use_empty=True)
bpy.ops.import_scene.gltf(filepath=in_path)
scene = bpy.context.scene

meshes = [o for o in scene.objects if o.type == "MESH"]
if not meshes:
    print(f"render-preview: ERROR {in_path} imported no meshes")
    sys.exit(1)
corners = [o.matrix_world @ Vector(c) for o in meshes for c in o.bound_box]
lo = tuple(min(c[i] for c in corners) for i in range(3))
hi = tuple(max(c[i] for c in corners) for i in range(3))
pose = plan_preview(lo, hi, azimuth, elevation, fov_deg=FOV_DEG, aspect=WIDTH / HEIGHT)

cam_data = bpy.data.cameras.new("PreviewCamera")
cam_data.sensor_fit = "VERTICAL"
cam_data.angle_y = math.radians(FOV_DEG)
cam_data.clip_start = pose["distance"] * 0.01
cam_data.clip_end = pose["distance"] * 10.0
camera = bpy.data.objects.new("PreviewCamera", cam_data)
scene.collection.objects.link(camera)
camera.location = Vector(pose["camera"])
camera.rotation_euler = (Vector(pose["look_at"]) - camera.location).to_track_quat("-Z", "Y").to_euler()
scene.camera = camera

world = bpy.data.worlds.new("PreviewBackdrop")
world.color = BACKDROP
scene.world = world

scene.render.engine = "BLENDER_WORKBENCH"
shading = scene.display.shading
shading.light = "STUDIO"
shading.color_type = "TEXTURE"
shading.show_specular_highlight = True
scene.display.render_aa = "8"
scene.render.resolution_x = WIDTH
scene.render.resolution_y = HEIGHT
scene.render.resolution_percentage = 100
scene.render.image_settings.file_format = "PNG"
scene.render.filepath = out_path
bpy.ops.render.render(write_still=True)

print(f"render-preview: {os.path.basename(in_path)} {len(meshes)} meshes, bounds "
      f"{tuple(round(v, 3) for v in lo)}..{tuple(round(v, 3) for v in hi)}, "
      f"camera az {azimuth:g} el {elevation:g} at {pose['distance']:.2f} -> {out_path}")
