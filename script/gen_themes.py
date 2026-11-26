#!/usr/bin/env python3
"""Generate assets/themes/orion.json from compact palettes.

Each theme is defined by a handful of base colors; every UI token and syntax
color is derived from them so all themes stay consistent.
Run: python3 script/gen_themes.py
"""
import json
import os

THEMES = [
    # name, mode, bg, panel (sidebar/title), border, fg, muted, accent, syntax
    dict(name="Orion", mode="dark", bg="#0e0f13", panel="#0a0b0e", border="#1d1f26",
         fg="#d8d9df", muted="#6c7080", accent="#f2b36d",
         syntax=dict(keyword="#c4a0ff", function="#8ab4ff", string="#a8d995", number="#f2b36d",
                     type="#ffd27a", constant="#f2b36d", comment="#4f5464", tag="#ff8a8a",
                     property="#9cc7ff", operator="#89ddff", punct="#868b9a")),
    dict(name="Nebula", mode="dark", bg="#110f1c", panel="#0c0a15", border="#231f35",
         fg="#dcd7ee", muted="#7a7396", accent="#b48cff",
         syntax=dict(keyword="#ff8bd0", function="#9bb8ff", string="#b5e6a3", number="#ffb38a",
                     type="#e6c0ff", constant="#ffb38a", comment="#5a5378", tag="#ff8bd0",
                     property="#a9c8ff", operator="#8be9fd", punct="#8c86a8")),
    dict(name="Aurora", mode="dark", bg="#0b1214", panel="#080e10", border="#18252a",
         fg="#d3e4e4", muted="#65807f", accent="#4fd8b8",
         syntax=dict(keyword="#7dd8ff", function="#7ee8c3", string="#d6e88a", number="#ffb27a",
                     type="#9fd5ff", constant="#ffb27a", comment="#4a6262", tag="#ff8f8f",
                     property="#a6e3d6", operator="#7dd8ff", punct="#7d9898")),
    dict(name="Midnight", mode="dark", bg="#0f1116", panel="#0b0d11", border="#1c1f27",
         fg="#d5d8e0", muted="#6b7180", accent="#7c9cff",
         syntax=dict(keyword="#c49bff", function="#82aaff", string="#a8d995", number="#f0a36e",
                     type="#ffcb6b", constant="#f0a36e", comment="#545a68", tag="#f2727f",
                     property="#9cc7ff", operator="#89ddff", punct="#8a90a0")),
    dict(name="Supernova", mode="dark", bg="#140f0e", panel="#0f0b0a", border="#2a201d",
         fg="#eadcd6", muted="#8a7670", accent="#ff7a59",
         syntax=dict(keyword="#ff9f7a", function="#ffd28a", string="#c3e08f", number="#ff7a59",
                     type="#ffc0a8", constant="#ff7a59", comment="#64544f", tag="#ff7a8a",
                     property="#f4c7a1", operator="#ffb38a", punct="#9a8780")),
    dict(name="Lunar", mode="light", bg="#fbfbfc", panel="#f4f5f7", border="#e5e7eb",
         fg="#1d2130", muted="#868b98", accent="#5b6cf0",
         syntax=dict(keyword="#8b3fd9", function="#2f5bea", string="#2f8a3a", number="#c2541b",
                     type="#b5780b", constant="#c2541b", comment="#9aa0ad", tag="#d0364b",
                     property="#1f6fb2", operator="#0f8fa8", punct="#6b7080")),
]


def hex_to_rgb(h):
    h = h.lstrip("#")
    return tuple(int(h[i:i + 2], 16) for i in (0, 2, 4))


def rgb_to_hex(rgb):
    return "#" + "".join(f"{max(0, min(255, round(c))):02x}" for c in rgb)


def mix(a, b, t):
    ra, rb = hex_to_rgb(a), hex_to_rgb(b)
    return rgb_to_hex(tuple(x + (y - x) * t for x, y in zip(ra, rb)))


def alpha(h, a):
    return h + f"{round(a * 255):02x}"


def build(t):
    dark = t["mode"] == "dark"
    bg, panel, border, fg, muted, acc = t["bg"], t["panel"], t["border"], t["fg"], t["muted"], t["accent"]
    on_acc = "#0b0c10" if dark else "#ffffff"
    # Chrome sits on a blurred, translucent window, so interactive fills are
    # translucent overlays rather than opaque mixes of the background.
    ov = "#ffffff" if dark else "#000000"
    hover = alpha(ov, 0.05 if dark else 0.045)
    active = alpha(ov, 0.09 if dark else 0.07)
    surface = alpha(ov, 0.06 if dark else 0.04)
    line = alpha(ov, 0.075 if dark else 0.085)
    tint = alpha(panel, 0.72 if dark else 0.62)
    s = t["syntax"]
    colors = {
        "background": bg, "foreground": fg, "border": line, "window.border": alpha(ov, 0.12),
        "ring": acc, "caret": acc, "overlay": "#00000066" if dark else "#0000002e",
        "selection.background": alpha(acc, 0.28),
        "accent.background": active, "accent.foreground": fg,
        "primary.background": acc, "primary.hover.background": mix(acc, "#ffffff", 0.12),
        "primary.active.background": mix(acc, "#000000", 0.1), "primary.foreground": on_acc,
        "secondary.background": surface, "secondary.hover.background": hover,
        "secondary.active.background": active, "secondary.foreground": mix(fg, bg, 0.08),
        "muted.background": surface, "muted.foreground": muted, "input.border": alpha(ov, 0.11),
        "popover.background": mix(bg, fg, 0.05) if dark else "#ffffff", "popover.foreground": fg,
        "list.background": "#00000000", "list.hover.background": hover,
        "list.active.background": alpha(acc, 0.20), "list.active.border": alpha(acc, 0),
        "list.even.background": "#00000000", "list.head.background": "#00000000",
        "list.highlight.foreground": acc,
        "sidebar.background": tint, "sidebar.foreground": mix(fg, bg, 0.12), "sidebar.border": line,
        "sidebar.accent.background": hover, "sidebar.accent.foreground": fg,
        "sidebar.primary.background": acc, "sidebar.primary.foreground": on_acc,
        "tab_bar.background": "#00000000", "tab_bar.segmented.background": surface,
        "tab.background": "#00000000", "tab.foreground": muted,
        "tab.active.background": active, "tab.active.foreground": fg,
        "title_bar.background": "#00000000", "title_bar.border": "#00000000",
        "status_bar.background": "#00000000", "status_bar.border": line,
        "scrollbar.background": "#00000000",
        "scrollbar.thumb.background": "#ffffff1a" if dark else "#00000022",
        "scrollbar.thumb.hover.background": "#ffffff30" if dark else "#00000036",
        "table.background": bg, "table.head.background": panel, "table.head.foreground": muted,
        "table.hover.background": hover, "table.row.border": line, "table.even.background": bg,
        "link.foreground": acc, "link.hover.foreground": mix(acc, fg, 0.3), "link.active.foreground": acc,
        "switch.background": alpha(ov, 0.16), "skeleton.background": hover,
        "drag_border": acc, "drop_target.background": alpha(acc, 0.1),
        "progress_bar.background": acc, "slider.bar.background": acc,
        "success.background": "#3fb98a", "success.foreground": on_acc,
        "warning.background": "#e5b567", "warning.foreground": "#0b0c10",
        "danger.background": "#f2727f", "danger.foreground": on_acc,
        "info.background": "#7c9cff", "info.foreground": on_acc,
        "base.red": "#f2727f", "base.green": "#7fd19b", "base.yellow": "#e5c07b",
        "base.blue": "#7c9cff", "base.magenta": "#c49bff", "base.cyan": "#6fd3e6",
    }
    syn = lambda c, **kw: {"color": c, **kw}
    highlight = {
        "editor.foreground": fg, "editor.background": bg,
        "editor.active_line.background": "#ffffff07" if dark else "#0000000a",
        "editor.line_number": mix(bg, fg, 0.22), "editor.active_line_number": mix(fg, bg, 0.25),
        "editor.invisible": "#ffffff1a" if dark else "#00000022",
        "conflict": "#f2727f", "created": "#7fd19b", "deleted": "#f2727f", "error": "#f2727f",
        "hidden": muted, "hint": "#6fd3e6", "ignored": mix(muted, bg, 0.3), "info": "#7c9cff",
        "modified": "#e5c07b", "predictive": mix(muted, bg, 0.15), "renamed": "#7c9cff",
        "success": "#7fd19b", "unreachable": mix(muted, bg, 0.15), "warning": "#e5c07b",
        "syntax": {
            "attribute": syn(s["type"]), "boolean": syn(s["number"]),
            "comment": syn(s["comment"], font_style="italic"),
            "comment.doc": syn(mix(s["comment"], fg, 0.15), font_style="italic"),
            "constant": syn(s["constant"]), "constructor": syn(s["type"]), "embedded": syn(fg),
            "function": syn(s["function"]), "keyword": syn(s["keyword"]),
            "link_text": syn(s["function"]), "link_uri": syn(s["operator"]),
            "number": syn(s["number"]), "operator": syn(s["operator"]),
            "punctuation": syn(s["punct"]), "punctuation.bracket": syn(s["punct"]),
            "punctuation.delimiter": syn(s["punct"]), "property": syn(s["property"]),
            "string": syn(s["string"]), "string.escape": syn(s["operator"]),
            "string.regex": syn(s["operator"]), "string.special": syn(s["operator"]),
            "string.special.symbol": syn(s["number"]), "tag": syn(s["tag"]),
            "text.literal": syn(s["string"]), "title": syn(s["function"], font_weight=700),
            "type": syn(s["type"]), "variable": syn(fg), "variable.special": syn(s["tag"]),
        },
    }
    return {
        "name": t["name"], "mode": t["mode"],
        "font.size": 13, "mono_font.family": "JetBrains Mono", "mono_font.size": 13, "radius": 6,
        "colors": colors, "highlight": highlight,
    }


out = {"name": "Orion", "author": "Orion IDE", "themes": [build(t) for t in THEMES]}
root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
path = os.path.join(root, "assets", "themes", "orion.json")
with open(path, "w") as f:
    json.dump(out, f, indent=2)
print("wrote", path, "with", len(THEMES), "themes")
