#!/usr/bin/env python3
"""Writes tests/models/assets/models/pair.glb: two boxes (a red and a blue one, each its own mesh and material) under
one node, made here so the test project has no third-party assets. Run from the repository root."""
import json, struct, pathlib

def box(hx, hy, hz):
    pos, nor, idx = [], [], []
    for axis in range(3):
        for sign in (-1, 1):
            n = [0, 0, 0]; n[axis] = sign
            u, v = [(1, 2), (2, 0), (0, 1)][axis]
            base = len(pos)
            for a, b in ((-1, -1), (1, -1), (1, 1), (-1, 1)):
                p = [0, 0, 0]; p[axis] = sign * (hx, hy, hz)[axis]
                p[u] = a * (hx, hy, hz)[u]; p[v] = b * (hx, hy, hz)[v]
                pos.append(p); nor.append(n)
            order = (0, 1, 2, 0, 2, 3) if sign > 0 else (0, 2, 1, 0, 3, 2)
            idx += [base + i for i in order]
    return pos, nor, idx

pos, nor, idx = box(0.5, 0.5, 0.5)
blob = b"".join(struct.pack("<3f", *p) for p in pos) + b"".join(struct.pack("<3f", *n) for n in nor) + b"".join(struct.pack("<H", i) for i in idx)
pb, nb = len(pos) * 12, len(nor) * 12
lo = [min(p[i] for p in pos) for i in range(3)]; hi = [max(p[i] for p in pos) for i in range(3)]
gltf = {
    "asset": {"version": "2.0"},
    "scene": 0, "scenes": [{"nodes": [0]}],
    "nodes": [{"name": "Pair", "children": [1, 2]},
              {"name": "Red box", "mesh": 0, "translation": [-1.2, 0.5, 0]},
              {"name": "Tall blue box", "mesh": 1, "translation": [1.2, 1.0, 0], "scale": [1, 2, 1]}],
    "meshes": [{"name": "Red", "primitives": [{"attributes": {"POSITION": 0, "NORMAL": 1}, "indices": 2, "material": 0}]},
               {"name": "Blue", "primitives": [{"attributes": {"POSITION": 0, "NORMAL": 1}, "indices": 2, "material": 1}]}],
    "materials": [{"name": "Red", "pbrMetallicRoughness": {"baseColorFactor": [0.9, 0.15, 0.1, 1], "metallicFactor": 0, "roughnessFactor": 0.7}},
                  {"name": "Blue", "pbrMetallicRoughness": {"baseColorFactor": [0.1, 0.3, 0.9, 1], "metallicFactor": 0, "roughnessFactor": 0.7}}],
    "accessors": [{"bufferView": 0, "componentType": 5126, "count": len(pos), "type": "VEC3", "min": lo, "max": hi},
                  {"bufferView": 1, "componentType": 5126, "count": len(nor), "type": "VEC3"},
                  {"bufferView": 2, "componentType": 5123, "count": len(idx), "type": "SCALAR"}],
    "bufferViews": [{"buffer": 0, "byteOffset": 0, "byteLength": pb}, {"buffer": 0, "byteOffset": pb, "byteLength": nb}, {"buffer": 0, "byteOffset": pb + nb, "byteLength": len(idx) * 2}],
    "buffers": [{"byteLength": len(blob)}],
}
j = json.dumps(gltf, separators=(",", ":")).encode(); j += b" " * (-len(j) % 4)
blob += b"\0" * (-len(blob) % 4)
out = struct.pack("<4sII", b"glTF", 2, 12 + 8 + len(j) + 8 + len(blob)) + struct.pack("<I4s", len(j), b"JSON") + j + struct.pack("<I4s", len(blob), b"BIN\0") + blob
p = pathlib.Path("tests/models/assets/models/pair.glb"); p.parent.mkdir(parents=True, exist_ok=True); p.write_bytes(out)
print(p, len(out), "bytes")
