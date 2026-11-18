import os
import sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from base import *
from icons import ic

OUT = os.path.join(os.path.dirname(os.path.abspath(__file__)), 'out') + os.sep
os.makedirs(OUT, exist_ok=True)
t = THEMES['Level']
A, A2 = t['acc'], t['acc2']


def card(title, purpose, body, span=1):
    return (f'<section style="grid-column: span {span}; display: flex; flex-direction: column; gap: 16px; padding: 22px; border-radius: 16px; background: #141417; box-shadow: inset 0 0 0 1px rgba(255,255,255,0.06)">'
            f'<div><h2 style="margin: 0; font-size: 16px; font-weight: 700; letter-spacing: -0.01em">{title}</h2>'
            f'<p style="margin: 4px 0 0; font-size: 12.5px; color: #A0A0A7; line-height: 1.45">{purpose}</p></div>{body}</section>')


def state(name, html, pad=True):
    p = 'padding-left: 18px;' if pad else ''
    return (f'<div style="display: flex; flex-direction: column; gap: 8px"><span style="font-size: 11.5px; font-weight: 500; color: #8E8E95; letter-spacing: .02em">{name}</span>'
            f'<div style="{p}">{html}</div></div>')


# ---------- word slider ----------
def slider_drag():
    stops = ['Feather', 'Light', 'Medium', 'Heavy', 'Fixed']
    idx, n = 3, 5
    pct = idx / (n - 1) * 100
    ticks = ''.join(f'<span class="tk" style="left: {i/(n-1)*100:.1f}%"></span>' for i in range(n))
    labs = ''.join(f'<span{" class=\"on\"" if i == idx else ""} style="{"left: 0" if i == 0 else ("right: 0" if i == n-1 else f"left: {i/(n-1)*100:.1f}%; transform: translateX(-50%)")}">{s}</span>' for i, s in enumerate(stops))
    return (f'<div class="row wsr"><span class="lbl">Weight</span><div class="ws"><div class="trk" style="height: 6px; margin-top: 7px">'
            f'<div class="fil" style="width: {pct:.1f}%"></div>{ticks}'
            f'<span style="position: absolute; left: {pct:.1f}%; top: -30px; transform: translateX(-50%); height: 22px; padding: 0 9px; border-radius: 11px; background: {A2}; color: {INK}; font-size: 12px; font-weight: 700; display: flex; align-items: center; white-space: nowrap">Heavy</span>'
            f'<span class="th" style="left: {pct:.1f}%; top: -7px; width: 20px; height: 20px; margin-left: -10px; border-radius: 10px; box-shadow: 0 0 0 5px {hexa(A2, 0.35)}"></span></div>'
            f'<div class="stp">{labs}</div></div></div>')


slider = card('Word slider', 'Feel settings use words first. Drag or click a word; the thumb snaps to the stops.',
              state('Rest', ws('Weight', ['Feather', 'Light', 'Medium', 'Heavy', 'Fixed'], 2))
              + state('Dragging — the value rides above the thumb', '<div style="padding-top: 26px">' + slider_drag() + '</div>')
              + state('Changed from default — bar marks it, reset appears on hover',
                      ws('Bounce', ['None', 'Soft', 'Springy'], 2, True).replace('</div></div></div>', f'</div></div><button type="button" class="ib" aria-label="Reset Bounce" title="Reset to None" style="color: {A2}">{ic("reset", "currentColor", 15)}</button></div>', 1))
              + state('With an icon — 14 px before the label, in its colour (white when changed)',
                      ws('Volume', ['Whisper', 'Quiet', 'Normal', 'Loud'], 2, icon='volume') + more_btn(A2))
              + state('More open — its caret turns down; exact value under the words',
                      ws('Weight', ['Feather', 'Light', 'Medium', 'Heavy', 'Fixed'], 3) + more_btn(A2, True)
                      + f'<div class="row"><span class="lbl"></span><div class="ax" style="flex: none; width: 120px"><span style="flex: 1">120</span><span style="color: #8E8E95; padding-right: 9px">kg</span></div><span style="font-size: 12px; color: #8E8E95">Typing a number moves the word</span></div>'))

# ---------- number fields ----------
def axf(letter, val, color, mode=''):
    if mode == 'hover':
        return (f'<div class="ax" style="background: #26262B"><b style="background: {color}">{letter}</b><span style="flex: 1">{val}</span>'
                f'<span style="padding-right: 6px; color: {A2}; display: flex">{ic("scrub", "currentColor", 14)}</span></div>')
    if mode == 'edit':
        return (f'<div class="ax" style="background: #17171A; box-shadow: inset 0 0 0 1.5px {A2}"><b style="background: {color}">{letter}</b>'
                f'<span style="flex: 1"><span style="background: {hexa(A2, 0.35)}; padding: 1px 0">-4.50</span><span style="display: inline-block; width: 1.5px; height: 14px; margin-left: 1px; vertical-align: -2px; background: #FFFFFF"></span></span></div>')
    return f'<div class="ax"><b style="background: {color}">{letter}</b><span>{val}</span></div>'


lockbtn = lambda on: (f'<button type="button" class="ib" aria-pressed="{str(on).lower()}" aria-label="Keep proportions" title="Keep proportions" style="min-width: 24px; width: 24px; padding: 0; color: {A2 if on else "#8E8E95"}">{ic("lock" if on else "unlock", "currentColor", 15)}</button>')
numbers = card('Number fields', 'For placement only. Drag sideways to scrub, double-click to type, Tab moves to the next axis.',
               state('Rest', f'<div class="row"><span class="lbl">Position</span>{axf("X", "-2.00", AX[0])}{axf("Y", "0.00", AX[1])}{axf("Z", "-4.50", AX[2])}</div>')
               + state('Hover — scrub arrows show the field can be dragged', f'<div class="row"><span class="lbl">Position</span>{axf("X", "-2.00", AX[0])}{axf("Y", "0.00", AX[1], "hover")}{axf("Z", "-4.50", AX[2])}</div>')
               + state('Typing — accent ring, whole value selected', f'<div class="row"><span class="lbl">Position</span>{axf("X", "-2.00", AX[0])}{axf("Y", "0.00", AX[1])}{axf("Z", "", AX[2], "edit")}</div>')
               + state('Linked — the lock keeps size in proportion', f'<div class="row"><span class="lbl" style="display: flex; align-items: center; justify-content: space-between">Size{lockbtn(True)}</span>{axf("X", "0.25", AX[0])}{axf("Y", "3.00", AX[1])}{axf("Z", "8.00", AX[2])}</div>'))

# ---------- colour picker ----------
cur = '#C2BAAA'
swatches = ['#C2BAAA', '#2B2B30', '#E9E7E2', '#8C5A3C', '#3E5A73', '#48704F', A, A2]
sw_html = ''.join(f'<button type="button" aria-label="Use {c}" style="width: 22px; height: 22px; border-radius: 6px; background: {c}; box-shadow: {"0 0 0 2px #1A1A1E, 0 0 0 4px " + A2 if c == cur else "inset 0 0 0 1px rgba(255,255,255,0.15)"}"></button>' for c in swatches)
picker_pop = f'''<div role="dialog" aria-label="Colour" style="width: 280px; padding: 14px; border-radius: 14px; background: #1A1A1E; box-shadow: inset 0 0 0 1px rgba(255,255,255,0.1), 0 18px 40px rgba(0,0,0,0.5); display: flex; flex-direction: column; gap: 12px; box-sizing: border-box">
<div style="display: flex; align-items: center; gap: 10px"><span style="width: 28px; height: 28px; border-radius: 8px; background: {cur}; box-shadow: inset 0 0 0 1px rgba(255,255,255,0.2)"></span><div style="flex: 1"><div style="font-size: 13px; font-weight: 700">Warm concrete</div><div style="font-size: 11.5px; color: #9C9CA3">Named automatically · click to rename</div></div></div>
<div style="position: relative; height: 150px; border-radius: 10px; background: linear-gradient(to top, #000000, rgba(0,0,0,0)), linear-gradient(to right, #FFFFFF, hsl(40, 100%, 50%))">
<span style="position: absolute; left: 12%; top: 24%; width: 16px; height: 16px; margin: -8px 0 0 -8px; border-radius: 8px; background: {cur}; box-shadow: 0 0 0 2.5px #FFFFFF, 0 2px 6px rgba(0,0,0,0.5)"></span></div>
<div style="position: relative; height: 12px; border-radius: 6px; background: linear-gradient(to right, #FF0000, #FFFF00, #00FF00, #00FFFF, #0000FF, #FF00FF, #FF0000)"><span style="position: absolute; left: 11%; top: 50%; width: 16px; height: 16px; margin: -8px 0 0 -8px; border-radius: 8px; background: hsl(40, 100%, 50%); box-shadow: 0 0 0 2.5px #FFFFFF"></span></div>
<div style="position: relative; height: 12px; border-radius: 6px; background: linear-gradient(to right, rgba(194,186,170,0), {cur}), repeating-conic-gradient(#3A3A40 0% 25%, #26262B 0% 50%) 0 0 / 8px 8px"><span style="position: absolute; left: 100%; top: 50%; width: 16px; height: 16px; margin: -8px 0 0 -8px; border-radius: 8px; background: {cur}; box-shadow: 0 0 0 2.5px #FFFFFF"></span></div>
<div style="display: flex; gap: 6px"><button type="button" class="ib" aria-label="Pick a colour from the view" title="Eyedropper  I" style="background: #26262B">{ic("pipette", "currentColor", 16)}</button>
<label class="inp" style="flex: 1; height: 28px"><span style="color: #8E8E95">Hex</span><input type="text" value="C2BAAA" aria-label="Hex colour" style="font-family: 'Roboto Mono', monospace"></label>
<label class="inp" style="width: 70px; height: 28px"><input type="text" value="100" aria-label="Opacity percent" style="font-family: 'Roboto Mono', monospace; text-align: right"><span style="color: #8E8E95">%</span></label></div>
<div style="display: flex; flex-direction: column; gap: 8px"><span style="font-size: 11.5px; color: #9C9CA3">In this project</span><div style="display: flex; gap: 6px; flex-wrap: wrap">{sw_html}<button type="button" aria-label="Save colour to project" style="width: 22px; height: 22px; border-radius: 6px; box-shadow: inset 0 0 0 1px #4A4A52; display: flex; align-items: center; justify-content: center; color: #B0B0B6">{ic("plus", "currentColor", 13)}</button></div></div>
</div>'''
picker = card('Colour picker', 'Opens from any swatch. Project colours come first so a game keeps one palette.',
              state('Closed — swatch, friendly name, opens on click', sw('Colour', cur, 'Warm concrete'))
              + state('Open', picker_pop, pad=False))

# ---------- toggles & choices ----------
def tgl_mixed():
    return f'<span class="tg" style="background: {hexa(A2, 0.5)}"><i style="left: 9px; top: 7px; width: 12px; height: 2px; border-radius: 1px; background: {INK}"></i></span>'


choices = card('Toggles and choices', 'Toggles for on/off, segmented for 2–4 options. More than four becomes a dropdown.',
               state('Toggle — on, off, and mixed when several parts disagree',
                     trow(t, 'Solid', True) + trow(t, 'Casts shadow', False) + f'<div class="row"><span class="lbl2">Visible <span style="color: #8E8E95; font-weight: 400">· 3 parts</span></span>{tgl_mixed()}</div>')
               + state('Toggle with its shortcut', trow(t, 'Dash', True, 'Shift'))
               + state('Segmented — words', seg('Moves on', ['Wheels', 'Hover', 'Tracks'], 0))
               + state('Segmented — icons, always with a tooltip',
                       '<div class="row"><span class="lbl">Shoulder</span><div class="sg" role="radiogroup" aria-label="Shoulder">'
                       + ''.join(f'<span title="{n}" class="{"on" if i == 2 else ""}">{ic(ico, "currentColor", 16)}</span>' for i, (n, ico) in enumerate([('Left', 'align_l'), ('Centre', 'align_ch'), ('Right', 'align_r')])) + '</div></div>'))

# ---------- dropdown ----------
menu_items = [('mountain', 'Level 1', True, ''), ('mountain', 'Level 2', False, ''), ('monitor', 'Options', False, ''), ('x', 'Close game', False, '')]
menu = ''.join(f'<button type="button" role="menuitemradio" aria-checked="{str(cur_).lower()}" class="srow" style="height: 32px{"; background: #26262B; color: #FFFFFF" if cur_ else ""}">{ic(ico, THEMES["Level"]["acc"] if ico == "mountain" else "#B0B0B6", 15)}{lab}<span style="margin-left: auto; display: flex; color: {A2}">{ic("check", "currentColor", 15, 2.5) if cur_ else ""}</span></button>' for ico, lab, cur_, k in menu_items)
dropdown = card('Dropdown', 'Lists of things in the game. Search appears once a list passes eight items.',
                state('Closed — leading icon shows what kind of thing it holds', dd('Opens', 'Level 1', ic('mountain', A, 16)))
                + state('Open', f'''<div role="menu" style="width: 100%; padding: 6px; border-radius: 12px; background: #1A1A1E; box-shadow: inset 0 0 0 1px rgba(255,255,255,0.1), 0 18px 40px rgba(0,0,0,0.5); box-sizing: border-box">
<label class="inp" style="margin-bottom: 6px">{ic("search", "#8E8E95", 14)}<input type="text" placeholder="Find a screen or level" aria-label="Find"></label>
{menu}<div style="height: 1px; margin: 6px 4px; background: rgba(255,255,255,0.08)"></div>
<button type="button" class="srow" style="color: {A2}">{ic("plus", "currentColor", 15)}New screen<span style="margin-left: auto"><span class="kc">N</span></span></button></div>''', pad=False))

# ---------- alignment ----------
def pin_grid(active):
    cells = ''
    for i in range(9):
        on = i == active
        inner = (f'<span style="width: 10px; height: 10px; border-radius: 3px; background: {A2}"></span>' if on
                 else '<span style="width: 4px; height: 4px; border-radius: 2px; background: #6E6E75"></span>')
        cells += f'<button type="button" aria-label="Pin {["top left","top","top right","left","centre","right","bottom left","bottom","bottom right"][i]}" aria-pressed="{str(on).lower()}" style="width: 24px; height: 24px; display: flex; align-items: center; justify-content: center; border-radius: 6px{"; background: " + hexa(A2, 0.16) if on else ""}">{inner}</button>'
    return f'<div style="display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 2px; padding: 4px; width: 84px; border-radius: 9px; background: #1F1F23">{cells}</div>'


def iconrow(items):
    return ('<div style="display: flex; padding: 2px; border-radius: 8px; background: #1F1F23">'
            + ''.join(f'<button type="button" class="ib" aria-label="{n}" title="{n}" style="height: 26px; min-width: 30px; padding: 0">{ic(i, "currentColor", 16)}</button>' for n, i in items) + '</div>')


align = card('Alignment', 'Pin a widget to a screen edge, then line parts up with each other.',
             state('Pin to — dot grid, the chosen spot fills', f'<div class="row" style="align-items: flex-start"><span class="lbl" style="padding-top: 4px">Pin to</span>{pin_grid(0)}<div style="display: flex; flex-direction: column; gap: 6px; padding-top: 4px"><span style="font-size: 12.5px; color: #FFFFFF; font-weight: 500">Top left</span>' + trow(t, 'Stretch across', False).replace('class="row"', 'class="row" style="min-height: 22px"') + '</div></div>')
             + state('Align and spread — appears when 2 or more parts are selected',
                     f'<div style="display: flex; flex-wrap: wrap; gap: 8px">{iconrow([("Align left", "align_l"), ("Align centres", "align_ch"), ("Align right", "align_r")])}'
                     f'{iconrow([("Align tops", "align_t"), ("Align middles", "align_cv"), ("Align bottoms", "align_b")])}{iconrow([("Spread across", "dist_h"), ("Spread down", "dist_v")])}</div>'))

# ---------- inputs ----------
inputs = card('Text inputs', 'Names, captions and search. Errors say what to do next.',
              state('Rest', f'<label class="inp"><span style="color: #8E8E95">Name</span><input type="text" value="Left wall" aria-label="Name"></label>')
              + state('Typing — accent ring', f'<label class="inp" style="box-shadow: inset 0 0 0 1.5px {A2}; background: #17171A"><span style="color: #8E8E95">Name</span><input type="text" value="Left wall 2" aria-label="Name, editing"></label>')
              + state('Problem — red ring and a fix', f'<label class="inp" style="box-shadow: inset 0 0 0 1.5px {DANGER}"><span style="color: #8E8E95">Name</span><input type="text" value="Door" aria-label="Name, has a problem"></label>'
                      f'<div style="display: flex; align-items: center; gap: 6px; margin-top: 6px; font-size: 12px; color: #FF8A98">{ic("alert", DANGER, 14)}Another part is already called Door<button type="button" style="margin-left: auto; color: {A2}; font-weight: 500">Use “Door 2”</button></div>')
              + state('Search — clear button appears once there is text', f'<label class="inp">{ic("search", "#8E8E95", 14)}<input type="text" value="light" aria-label="Find in scene"><button type="button" aria-label="Clear" style="display: flex; color: #8E8E95">{ic("x", "currentColor", 14)}</button></label>'))

# ---------- indicators ----------
spin = f'<svg width="14" height="14" viewBox="0 0 24 24" aria-hidden="true"><circle cx="12" cy="12" r="9" fill="none" stroke="rgba(255,255,255,0.2)" stroke-width="2.5"></circle><path d="M12 3 A9 9 0 0 1 21 12" fill="none" stroke="{A2}" stroke-width="2.5" stroke-linecap="round"></path></svg>'
indicators = card('Indicators', 'Small, consistent signals. Colour always comes with a word or icon.',
                  state('Save state', f'<div style="display: flex; gap: 16px; font-size: 12.5px"><span style="display: inline-flex; align-items: center; gap: 6px">{ic("check", CAT["flow"], 14, 2.5)}Saved</span>'
                        f'<span style="display: inline-flex; align-items: center; gap: 6px; color: #D2D2D6">{spin}Saving</span>'
                        f'<span style="display: inline-flex; align-items: center; gap: 6px; color: #D2D2D6"><span style="width: 8px; height: 8px; border-radius: 4px; background: {A}"></span>Unsaved changes</span></div>', pad=False)
                  + state('Counts and warnings', f'<div style="display: flex; flex-wrap: wrap; gap: 8px; align-items: center">'
                          f'<span style="display: inline-flex; align-items: center; gap: 6px; font-size: 12.5px">Wires<span style="min-width: 20px; height: 20px; padding: 0 6px; border-radius: 10px; background: {A}; color: {INK}; font-size: 11.5px; font-weight: 700; display: inline-flex; align-items: center; justify-content: center; box-sizing: border-box">3</span></span>'
                          f'<span style="display: inline-flex; align-items: center; gap: 6px; height: 24px; padding: 0 9px; border-radius: 12px; background: {hexa(DANGER, 0.16)}; color: #FF8A98; font-size: 12px; font-weight: 500">{ic("alert", DANGER, 13)}Too many lights</span>'
                          f'<span style="display: inline-flex; align-items: center; gap: 6px; height: 24px; padding: 0 9px; border-radius: 12px; background: rgba(255,255,255,0.07); color: #D2D2D6; font-size: 12px">{ic("info", "#B0B0B6", 13)}Hidden in game</span>'
                          f'<span style="display: inline-flex; align-items: center; gap: 6px; height: 24px; padding: 0 9px; border-radius: 12px; background: {hexa(DANGER, 0.16)}; color: #FF8A98; font-size: 12px; font-weight: 700"><span style="width: 8px; height: 8px; border-radius: 4px; background: {DANGER}; box-shadow: 0 0 0 3px {hexa(DANGER, 0.3)}"></span>Recording</span></div>', pad=False)
                  + state('Progress — always says what is happening', f'<div style="display: flex; flex-direction: column; gap: 6px"><div style="display: flex; justify-content: space-between; font-size: 12.5px"><span>Baking light</span><span style="font-family: \'Roboto Mono\', monospace; color: #B0B0B6">62%</span></div>'
                          f'<div style="height: 6px; border-radius: 3px; background: #2A2A2F"><div style="width: 62%; height: 6px; border-radius: 3px; background: {A2}"></div></div></div>', pad=False)
                  + state('Keycaps and tooltip', f'<div style="display: flex; align-items: center; gap: 14px; flex-wrap: wrap"><span class="kc">Ctrl Z</span>'
                          f'<span style="display: inline-flex; padding: 6px 8px; border-radius: 8px; background: {THEMES["Characters"]["bg"]}"><span class="kf" style="background: {INK}; color: {THEMES["Characters"]["bg"]}">Tab</span></span>'
                          f'<span class="tip">Duplicate <span class="kc">Ctrl D</span></span></div>', pad=False))

# ---------- buttons ----------
buttons = card('Buttons', 'One main action per panel. Labels are verbs.',
               state('Main · Second · Quiet · Danger',
                     f'<div style="display: flex; flex-wrap: wrap; gap: 8px; align-items: center">'
                     f'<button type="button" class="btn" style="background: {A}; color: {INK}">{ic("play", "currentColor", 14, 2.5)}Play here <span class="kc" style="background: {INK}; border-color: {INK}; color: {A}">`</span></button>'
                     f'<button type="button" class="btn" style="background: #2A2A2F; color: #F3F3F1">{ic("plus", "currentColor", 15)}Add wire</button>'
                     f'<button type="button" class="btn" style="color: {A2}; padding: 0 6px">{ic("plus", "currentColor", 15)}New sound</button>'
                     f'<button type="button" class="btn" style="background: {hexa(DANGER, 0.16)}; color: #FF8A98">{ic("trash", "currentColor", 15)}Delete 3 parts</button></div>', pad=False)
               + state('Icon buttons — tooltip with shortcut on hover',
                       f'<div style="display: flex; gap: 4px; align-items: center"><button type="button" class="ib" aria-label="Duplicate">{ic("copy", "currentColor", 16)}</button><button type="button" class="ib" aria-label="Delete">{ic("trash", "currentColor", 16)}</button>'
                       f'<button type="button" class="ib" aria-label="Reset" style="background: #26262B">{ic("reset", "currentColor", 16)}</button><span class="tip" style="margin-left: 8px">Reset to default <span class="kc">Alt Click</span></span></div>', pad=False)
               + state('Keyboard focus — accent ring on everything', f'<button type="button" class="btn" style="background: #2A2A2F; color: #F3F3F1; outline: 2px solid {A2}; outline-offset: 2px">Add wire</button>', pad=False))

body = (f'<div style="display: flex; align-items: baseline; gap: 20px"><h1 style="margin: 0; font-size: 32px; font-weight: 700; letter-spacing: -0.02em">Components</h1>'
        f'<span style="font-size: 15px; color: #B0B0B6">Shown in Level colours. In other tabs the yellow becomes the tab colour and the violet becomes the tab’s accent.</span></div>'
        f'<div style="display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 16px; align-items: start">'
        f'{slider}{numbers}{picker}{choices}{dropdown}{align}{inputs}{indicators}{buttons}</div>')

html = f'''<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>Beverly — components</title>
<script src="./support.js"></script>
</head>
<body>
<x-dc>
<helmet>
<link href="https://fonts.googleapis.com/css2?family=Roboto:wght@400;500;700&amp;family=Roboto+Mono:wght@400;500;700&amp;display=swap" rel="stylesheet">
<style>
{css(t)}</style>
</helmet>
<div style="width: 1600px; min-height: 1900px; box-sizing: border-box; padding: 56px; background: {INK}; font-family: Roboto, sans-serif; color: {TXT}; display: flex; flex-direction: column; gap: 28px">
{body}
</div>
</x-dc>
<script type="text/x-dc" data-dc-script data-props='{{"$preview":{{"width":1600,"height":1900}}}}'>
class Component extends DCLogic {{
  renderVals() {{ return {{}}; }}
}}
</script>
</body>
</html>
'''
open(OUT + 'Components.dc.html', 'w').write(html)
print('ok', len(html) // 1024, 'KB')
