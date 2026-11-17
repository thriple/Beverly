import os
import sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from base import *
from icons import ic
from scene import svg, room

OUT = os.path.join(os.path.dirname(os.path.abspath(__file__)), 'out') + os.sep
os.makedirs(OUT, exist_ok=True)


def write(name, html):
    open(OUT + name, 'w').write(html)


def status(t, text, icon='move', warn=False):
    w = ''
    if warn:
        w = (f'<div class="pill" style="box-shadow: inset 0 0 0 1px {hexa(DANGER, 0.7)}; color: #FF8A98">{ic("bulb", DANGER, 14)}Lights'
             f'<span style="display: flex; gap: 2px">' + ''.join(f'<i style="width: 8px; height: 10px; border-radius: 2px; background: {DANGER}"></i>' for _ in range(4)) +
             f'<i style="width: 8px; height: 10px; border-radius: 2px; background: {DANGER}; opacity: .45"></i></span><span style="font-family: \'Roboto Mono\', monospace">5 / 4</span></div>')
    return (f'<div style="position: absolute; left: 14px; top: 14px; display: flex; gap: 8px">'
            f'<div class="pill">{ic(icon, t["acc2"], 14)}{text}</div>{w}</div>')


def tag(t, x, y, text, icon):
    return (f'<div style="position: absolute; left: {x}px; top: {y}px; display: flex; align-items: center; gap: 6px; height: 24px; padding: 0 9px 0 7px; '
            f'border-radius: 7px; background: {t["acc"]}; color: {t["accFg"]}; font-size: 12px; font-weight: 700">{ic(icon, "currentColor", 13, 1.8)}{text}</div>')


def more(t):
    return f'<button type="button" style="margin-top: 14px; font-size: 12px; font-weight: 500; color: {t["acc2"]}">More numbers ›</button>'


# ---------------- timeline ----------------
PX, X0 = 126, 12
def tx(s): return X0 + s * PX


def key(s, row, c, sel=None):
    ring = f'box-shadow: 0 0 0 2px {INK}, 0 0 0 4px {sel};' if sel else ''
    return f'<span style="position: absolute; left: {tx(s)-5:.0f}px; top: {28+row*32+11}px; width: 10px; height: 10px; background: {c}; transform: rotate(45deg); border-radius: 2px; {ring}"></span>'


def span(a, b, row, c):
    return f'<span style="position: absolute; left: {tx(a):.0f}px; width: {(b-a)*PX:.0f}px; top: {28+row*32+15}px; height: 2px; background: {c}; opacity: .5"></span>'


def block(a, b, row, bg, fg, label, ring=None):
    r = f' box-shadow: 0 0 0 2px {INK}, 0 0 0 4px {ring};' if ring else ''
    return (f'<span style="position: absolute; left: {tx(a)+1:.0f}px; width: {(b-a)*PX-2:.0f}px; top: {28+row*32+4}px; height: 24px; border-radius: 6px; '
            f'background: {bg}; color: {fg}; font-size: 12px; font-weight: 500; display: flex; align-items: center; padding-left: 9px; box-sizing: border-box; overflow: hidden; white-space: nowrap;{r}">{label}</span>')


def timeline(t, crumbs, names, lanes, ph, sel_row=None, footer=''):
    rows = []
    for i, (c, n, p, icon) in enumerate(names):
        bg = 'rgba(255,255,255,0.05)' if i == sel_row else 'transparent'
        rows.append(f'<div style="display: flex; align-items: center; gap: 9px; height: 32px; padding: 0 12px 0 16px; background: {bg}; font-size: 12.5px">{ic(icon, c, 14)}<span style="color: #F3F3F1">{n}</span><span style="color: #9C9CA3">{p}</span></div>')
    ticks = ''.join(f'<span style="position: absolute; left: {tx(s):.0f}px; top: 7px; font: 11px \'Roboto Mono\', monospace; color: #9C9CA3; transform: translateX(-50%)">{s}s</span>'
                    f'<span style="position: absolute; left: {tx(s):.0f}px; top: 23px; width: 1px; height: 5px; background: #4A4A52"></span>' for s in range(11))
    sel = f'<div style="position: absolute; left: 0; right: 0; top: {28+sel_row*32}px; height: 32px; background: rgba(255,255,255,0.05)"></div>' if sel_row is not None else ''
    p = tx(ph)
    head = (f'<span style="position: absolute; left: {p:.0f}px; top: 0; bottom: 0; width: 2px; margin-left: -1px; background: {t["acc2"]}"></span>'
            f'<span style="position: absolute; left: {p-26:.0f}px; top: 3px; width: 52px; height: 20px; border-radius: 6px; background: {t["acc2"]}; color: {INK}; font: 700 11px/20px \'Roboto Mono\', monospace; text-align: center">{ph:.2f}</span>')
    crumb = '<span style="color: #6E6E75">/</span>'.join(
        (f'<span style="color: #F3F3F1; font-weight: 700">{c}</span>' if i == len(crumbs) - 1 else f'<a href="#" style="color: #9C9CA3; text-decoration: none">{c}</a>')
        for i, c in enumerate(crumbs))
    return f'''<section aria-label="Timeline" class="glass" style="position: absolute; left: 12px; right: 68px; bottom: 46px; height: 300px; display: flex; flex-direction: column; border-radius: 16px">
<button type="button" aria-label="Drag to resize timeline" style="position: absolute; left: 50%; top: -9px; transform: translateX(-50%); width: 72px; height: 18px; display: flex; align-items: center; justify-content: center"><span style="width: 56px; height: 6px; border-radius: 3px; background: #F3F3F1; opacity: .85"></span></button>
<div style="display: flex; align-items: center; gap: 14px; height: 46px; padding: 0 12px 0 16px; border-bottom: 1px solid rgba(255,255,255,0.07)">
<nav aria-label="Breadcrumb" style="display: flex; align-items: center; gap: 8px; font-size: 13px; width: 204px">{ic('clock', t['acc2'], 16)}{crumb}</nav>
<button class="ib" type="button" aria-label="Back to start" title="Back to start  Home">{ic('start', 'currentColor', 16)}</button>
<button type="button" aria-label="Play" title="Play  Enter" style="display: inline-flex; align-items: center; gap: 7px; height: 28px; padding: 0 13px 0 11px; border-radius: 8px; background: {t['acc']}; color: {t['accFg']}; font-size: 12.5px; font-weight: 700">{ic('play', 'currentColor', 14, 2.5)}Play</button>
<button class="ib" type="button" aria-label="Loop" title="Loop  L">{ic('loop', 'currentColor', 16)}</button>
<span style="font: 500 13px 'Roboto Mono', monospace; color: #F3F3F1">0:{ph:05.2f} <span style="color: #8E8E95">/ 0:10.00</span></span>
<div style="margin-left: auto; display: flex; align-items: center; gap: 10px; font-size: 12px; color: #9C9CA3">Seconds<span style="position: relative; width: 90px; height: 4px; border-radius: 2px; background: #34343A"><span style="position: absolute; left: 0; width: 35%; height: 4px; border-radius: 2px; background: {t['acc2']}"></span><span style="position: absolute; left: 35%; top: -6px; width: 16px; height: 16px; margin-left: -8px; border-radius: 8px; background: #FFFFFF"></span></span>Frames</div>
<button class="ib" type="button" aria-label="Close timeline" title="Close  T">{ic('x', 'currentColor', 16)}</button>
</div>
<div style="flex: 1; display: flex; min-height: 0">
<div style="width: 220px; flex: none; padding-top: 28px; border-right: 1px solid rgba(255,255,255,0.07)">{"".join(rows)}</div>
<div style="position: relative; flex: 1; overflow: hidden">{sel}{ticks}{"".join(lanes)}{head}</div>
</div>
<div style="display: flex; align-items: center; gap: 4px; height: 42px; padding: 0 8px; border-top: 1px solid rgba(255,255,255,0.07)">
<button class="ib" type="button" aria-pressed="true" title="Auto key  K" style="padding: 0 8px; gap: 8px">{ic('diamond', t['acc2'], 14)}Auto key {tg(t, True)}</button>
<span style="width: 1px; height: 16px; background: #34343A; margin: 0 4px"></span>
{footer}
<button class="ib" type="button" style="padding: 0 8px">Track ▾</button><button class="ib" type="button" style="padding: 0 8px">View ▾</button><button class="ib" type="button" style="padding: 0 8px">Options ▾</button>
</div>
</section>'''


def ease(t, s, top):
    return (f'<div style="position: absolute; left: {tx(s)+18:.0f}px; top: {top}px; display: flex; align-items: center; gap: 4px; padding: 5px 6px 5px 11px; border-radius: 10px; background: #2A2A2F; box-shadow: 0 6px 18px rgba(0,0,0,.45); font-size: 12px">'
            f'<span style="color: #B0B0B6; margin-right: 4px">Ease in</span>'
            f'<button type="button" style="height: 24px; padding: 0 9px; border-radius: 6px; color: #D2D2D6">Sudden</button>'
            f'<button type="button" style="height: 24px; padding: 0 9px; border-radius: 6px; background: {t["acc"]}; color: {t["accFg"]}; font-weight: 700">Gentle</button>'
            f'<button type="button" style="height: 24px; padding: 0 9px; border-radius: 6px; color: #D2D2D6">Floaty</button></div>')


def addkey(label, k, icon, t):
    return f'<button class="ib" type="button" title="{label}  {k}" style="padding: 0 8px">{ic(icon, "#B0B0B6", 13)}{label}</button>'


LEVEL_TOPICS = ['All', 'Physics', 'Lighting', 'Cameras', 'Water*', 'Foliage*', 'World']


# ---------------- LEVEL ----------------
def level():
    t = THEMES['Level']
    ins = (ident(CAT['shape'], 'cube', 'Left wall', 'Shape · Box · in <a href="#" style="color: #D2D2D6">Room</a>')
           + grp('Placement', axrow('Position', ['-2.00', '0.00', '-4.50']) + axrow('Turn', ['0°', '90°', '0°']) + axrow('Size', ['0.25', '3.00', '8.00']), 'move', CAT['shape'])
           + grp('Look', sw('Colour', '#C2BAAA', 'Warm concrete') + dd('Material', 'Concrete') + ws('Surface', ['Glossy', 'Satin', 'Matte'], 2, True), 'drop', CAT['light'])
           + grp('Feel', trow(t, 'Solid', True) + ws('Weight', ['Feather', 'Light', 'Medium', 'Heavy', 'Fixed'], 4, True) + ws('Bounce', ['None', 'Soft', 'Springy'], 0) + more(t), 'feel', CAT['flow'])
           + grp('Wires', f'<div class="row"><span class="lbl2" style="color: #9C9CA3">No wires yet</span><button type="button" class="ib" style="background: #2A2A2F; padding: 0 12px">{ic("plus", "currentColor", 14)}Add wire</button></div>', 'wire', CAT['act'])
           + grp('Advanced', '', 'sliders', '#8E8E95', '4 settings'))
    L = lst([('Room', [(CAT['shape'], 'Left wall', True, ''), (CAT['shape'], 'Right wall', False, ''), (CAT['shape'], 'Back wall', False, ''), (CAT['shape'], 'Floor', False, '')]),
             ('Door', [(CAT['shape'], 'Door', False, ''), (CAT['trig'], 'Door trigger', False, '', 'bolt'), (CAT['act'], 'Open door', False, '', 'play')]),
             ('Loose', [(CAT['game'], 'Player start 1', False, '', 'person'), (CAT['light'], 'Ceiling light', False, '!Too many lights', 'bulb'), (CAT['cam'], 'Camera 1', False, '', 'camera')])], 'Find in scene')
    view = (notch(t, LEVEL_TOPICS, 'Physics') + status(t, 'Editor movement', 'move', True) + tag(t, 386, 228, 'Left wall', 'cube') + dock(t, ins, L))
    write('Main.dc.html', page('Level', 'Beverly — Level', view, DOCK_SCRIPT, world=svg(room('wall', t['acc']))))


def level_timeline():
    t = THEMES['Level']
    names = [(CAT['trig'], 'Door trigger', 'fires', 'bolt'), (CAT['shape'], 'Door', 'Swing', 'cube'), (CAT['act'], 'Play sound', 'Creak', 'music'),
             (CAT['light'], 'Ceiling light', 'Brightness', 'bulb'), (CAT['cam'], 'Camera 1', 'Look at', 'camera')]
    lanes = [f'<span style="position: absolute; left: {tx(0):.0f}px; top: 32px; height: 24px; padding: 0 9px; border-radius: 6px; background: {CAT["trig"]}; color: {INK}; font-size: 12px; font-weight: 700; display: flex; align-items: center; gap: 6px">{ic("bolt", INK, 12, 1.8)}Player steps in</span>',
             span(.5, 2.0, 1, CAT['shape']), key(.5, 1, CAT['shape']), key(2.0, 1, CAT['shape']),
             block(.5, 2.2, 2, hexa(CAT['act'], 0.3), '#CFE0FF', 'creak.wav'),
             span(1.8, 3.0, 3, CAT['light']), key(1.8, 3, CAT['light']), key(2.1, 3, CAT['light']), key(2.4, 3, CAT['light']), key(3.0, 3, CAT['light']),
             span(0, 3.2, 4, CAT['cam']), key(0, 4, CAT['cam']), key(3.2, 4, CAT['cam'], t['acc']), ease(t, 3.2, 96)]
    view = (notch(t, LEVEL_TOPICS, 'All') + status(t, 'Editor movement') + tag(t, 990, 306, 'Door', 'cube') + rail(t)
            + timeline(t, ['Level', 'Door opens'], names, lanes, 3.2, 4, addkey('Add key', 'I', 'plus', t)))
    write('Timeline.dc.html', page('Level', 'Beverly — Level timeline', view, STATIC_SCRIPT, world=svg(room('door', t['acc']))))


# ---------------- CHARACTERS ----------------
def figure(x, fy, pose, body, op=1.0, outline=None):
    if pose == 'run':
        pts = dict(head=(8, -140), torso=[(4, -118), (0, -62)], arms=[[(4, -112), (24, -90), (38, -104)], [(4, -112), (-20, -92), (-30, -74)]],
                   legs=[[(0, -62), (22, -32), (18, 0)], [(0, -62), (-14, -30), (-34, -14)]])
    elif pose == 'tuck':
        pts = dict(head=(10, -132), torso=[(6, -110), (0, -58)], arms=[[(6, -104), (30, -118), (40, -138)], [(6, -104), (-18, -120), (-26, -140)]],
                   legs=[[(0, -58), (26, -48), (14, -22)], [(0, -58), (-6, -36), (-24, -26)]])
    else:
        pts = dict(head=(4, -124), torso=[(2, -102), (0, -50)], arms=[[(2, -96), (26, -78), (40, -70)], [(2, -96), (-22, -80), (-36, -72)]],
                   legs=[[(0, -50), (20, -26), (24, 0)], [(0, -50), (-18, -26), (-22, 0)]])

    def pl(seq, w, c):
        d = ' '.join(f'{x+a:.0f},{fy+b:.0f}' for a, b in seq)
        return f'<polyline points="{d}" fill="none" stroke="{c}" stroke-width="{w}" stroke-linecap="round" stroke-linejoin="round"></polyline>'

    def draw(c, extra):
        s = pl(pts['torso'], 24 + extra, c)
        for a in pts['arms'] + pts['legs']:
            s += pl(a, 12 + extra, c)
        hx, hy = pts['head']
        return s + f'<circle cx="{x+hx}" cy="{fy+hy}" r="{15 + extra/2:.0f}" fill="{c}"></circle>'
    return f'<g opacity="{op}">{draw(outline, 6) if outline else ""}{draw(body, 0)}</g>'


def characters():
    t = THEMES['Characters']
    inner = (f'<ellipse cx="600" cy="604" rx="150" ry="36" fill="{INK}" fill-opacity=".55"></ellipse>'
             f'<ellipse cx="600" cy="600" rx="150" ry="36" fill="none" stroke="{t["acc2"]}" stroke-width="2"></ellipse>'
             f'<path d="M600 590 Q 820 220 1040 590" fill="none" stroke="#FFFFFF" stroke-width="2.5" stroke-dasharray="7 6" opacity=".9"></path>'
             + figure(820, 450, 'tuck', '#FFFFFF', 0.45) + figure(1040, 596, 'land', '#FFFFFF', 0.45)
             + figure(600, 596, 'run', '#ECECEF', 1, t['acc']))
    ins = (ident(CAT['game'], 'person', 'Runner', 'Character · Player 1')
           + grp('Moves', ws('Run speed', ['Stroll', 'Jog', 'Run', 'Sprint'], 2) + ws('Jump', ['Hop', 'Jump', 'Leap'], 2, True) + ws('Air control', ['None', 'Some', 'Full'], 1), 'run', CAT['game'])
           + grp('Abilities', trow(t, 'Double jump', True, 'Space ×2') + trow(t, 'Dash', True, 'Shift', True) + trow(t, 'Wall climb', False) + trow(t, 'Swim', False), 'star', CAT['flow'])
           + grp('Camera', ws('Distance', ['Close', 'Mid', 'Far'], 1) + seg('Shoulder', ['Left', 'Centre', 'Right'], 2), 'camera', CAT['cam'])
           + grp('Advanced', '', 'sliders', '#8E8E95', '6 settings'))
    L = lst([('Characters', [(CAT['game'], 'Runner', True, 'Player 1', 'person'), (CAT['game'], 'Guard', False, '', 'person'), (CAT['game'], 'Drone pet', False, '', 'person')]),
             ('Runner’s abilities', [(CAT['flow'], 'Double jump', False, '', 'star'), (CAT['flow'], 'Dash', False, '', 'star'), ('#8E8E95', 'Wall climb', False, 'off', 'star'), ('#8E8E95', 'Swim', False, 'off', 'star')])], 'Find character')
    view = (notch(t, ['Body', 'Moves', 'Camera', 'Controls'], 'Moves') + status(t, 'Preview · Run and jump', 'play')
            + f'<div class="pill" style="position: absolute; left: 762px; top: 296px">{ic("run", t["acc2"], 14)}Leap</div>'
            + tag(t, 560, 412, 'Runner', 'person') + dock(t, ins, L, 'Cast', 'person'))
    write('Characters.dc.html', page('Characters', 'Beverly — Characters', view, DOCK_SCRIPT, [('Orbit', 'Middle-drag'), ('Pose', 'P'), ('Test moves', '`')], world=svg(inner)))


# ---------------- VEHICLES ----------------
def vehicles():
    t = THEMES['Vehicles']
    a = t['acc']
    body = '470,488 488,446 540,430 700,424 790,452 806,486'
    inner = f'''<ellipse cx="640" cy="532" rx="200" ry="26" fill="#1C2A16" opacity=".3"></ellipse>
<path d="M300 560 C 420 540, 520 536, 640 534" fill="none" stroke="#FFFFFF" stroke-width="2.5" stroke-dasharray="7 6" opacity=".7"></path>
<polyline points="548,432 584,360 694,360 724,426" fill="none" stroke="#C9CDD3" stroke-width="7" stroke-linejoin="round" stroke-linecap="round"></polyline>
<line x1="640" y1="360" x2="636" y2="428" stroke="#C9CDD3" stroke-width="5" stroke-linecap="round"></line>
<polygon points="{body}" fill="#26282E" stroke="{a}" stroke-width="3" stroke-linejoin="round"></polygon>
<polygon points="540,452 790,456 798,470 520,470" fill="{t['acc2']}"></polygon>
<rect x="596" y="398" width="34" height="30" rx="6" fill="#3A3D44"></rect>
<circle cx="796" cy="466" r="7" fill="{CAT['light']}" stroke="#FFFFFF" stroke-width="1.5"></circle>
<circle cx="536" cy="494" r="44" fill="#16171A"></circle><circle cx="536" cy="494" r="16" fill="#9A9EA6"></circle>
<circle cx="752" cy="498" r="42" fill="#16171A"></circle><circle cx="752" cy="498" r="15" fill="#9A9EA6"></circle>
<path d="M680 360 l0 -26" stroke="{CAT['cam']}" stroke-width="2" stroke-dasharray="3 3"></path>
<g transform="translate(668 318)" stroke="{CAT['cam']}" stroke-width="2.5" fill="{INK}" fill-opacity=".35" stroke-linejoin="round"><rect x="0" y="0" width="22" height="15" rx="3"></rect><path d="M22 4 l9 -4 v15 l-9 -4 Z"></path></g>'''
    ins = (ident(a, 'car', 'Dune buggy', 'Vehicle · 2 seats')
           + grp('Drive', seg('Moves on', ['Wheels', 'Hover', 'Tracks'], 0) + ws('Top speed', ['Slow', 'Brisk', 'Fast', 'Blistering'], 2, True) + ws('Pickup', ['Gentle', 'Punchy', 'Instant'], 1), 'wheel', a)
           + grp('Handling', ws('Grip', ['Icy', 'Loose', 'Firm', 'Glued'], 2) + ws('Weight', ['Light', 'Medium', 'Heavy'], 1) + trow(t, 'Handbrake drift', True, 'Space', True), 'gauge', CAT['flow'])
           + grp('Seats', dd('Driver', 'Player 1') + trow(t, 'Passenger can shoot', False), 'seat', CAT['game'])
           + grp('Advanced', '', 'sliders', '#8E8E95', '5 settings'))
    L = lst([('Garage', [(a, 'Dune buggy', True, '', 'car'), (a, 'Hover sled', False, '', 'car'), (a, 'Cargo truck', False, '', 'car')]),
             ('Buggy parts', [(CAT['shape'], 'Roll cage', False, ''), (CAT['shape'], 'Wheels', False, '×4', 'wheel'), (CAT['light'], 'Headlight', False, '', 'bulb'), (CAT['cam'], 'Chase camera', False, '', 'camera')])], 'Find vehicle')
    view = (notch(t, ['Body', 'Drive', 'Seats', 'Camera'], 'Drive') + status(t, 'Preview · Test path')
            + tag(t, 470, 396, 'Dune buggy', 'car') + dock(t, ins, L, 'Garage', 'car'))
    write('Vehicles.dc.html', page('Vehicles', 'Beverly — Vehicles', view, DOCK_SCRIPT, [('Orbit', 'Middle-drag'), ('Drive it', '`'), ('Reset', 'R')], world=svg(inner)))


# ---------------- UI / HUD ----------------
def hud():
    t = THEMES['UI/HUD']
    a = t['acc']
    hb_x, hb_y, hb_w, hb_h = 56, 56, 320, 24
    handles = ''.join(f'<span style="position: absolute; left: {hx - 4}px; top: {hy - 4}px; width: 8px; height: 8px; background: #FFFFFF; box-shadow: inset 0 0 0 2px {a}"></span>'
                      for hx in (hb_x - 6, hb_x + hb_w / 2, hb_x + hb_w + 6) for hy in (hb_y - 6, hb_y + hb_h / 2, hb_y + hb_h + 6) if not (hx == hb_x + hb_w / 2 and hy == hb_y + hb_h / 2))
    screen = f'''<div style="position: absolute; left: 40px; top: 70px; width: 1120px; height: 630px; border-radius: 12px; overflow: hidden; box-shadow: 0 0 0 1px rgba(255,255,255,0.16), 0 20px 50px rgba(0,0,0,0.45)">
{svg(room(None, a), 'h', '1120', '630', '200 40 1200 675')}
<div style="position: absolute; inset: 28px; border: 1.5px dashed rgba(255,255,255,0.55); border-radius: 4px"></div>
<svg width="1120" height="630" style="position: absolute; inset: 0" aria-hidden="true"><path d="M28 28 L{hb_x-6} {hb_y-6}" stroke="{t['acc2']}" stroke-width="2" stroke-dasharray="4 3"></path><circle cx="28" cy="28" r="5" fill="{t['acc2']}" stroke="#FFFFFF" stroke-width="1.5"></circle></svg>
<div style="position: absolute; left: {hb_x}px; top: {hb_y}px; width: {hb_w}px; height: {hb_h}px; border-radius: 12px; background: rgba(14,14,16,0.6); overflow: hidden"><div style="width: 72%; height: 100%; border-radius: 12px; background: {CAT['flow']}"></div></div>
<div style="position: absolute; left: {hb_x-6}px; top: {hb_y-6}px; width: {hb_w+12}px; height: {hb_h+12}px; box-shadow: inset 0 0 0 2px {a}; border-radius: 4px"></div>
{handles}
<div style="position: absolute; left: 50%; top: 50px; transform: translateX(-50%); height: 34px; padding: 0 16px; border-radius: 17px; background: rgba(14,14,16,0.6); display: flex; align-items: center; font-size: 16px; font-weight: 500; color: #FFFFFF">Reach the beacon</div>
<div style="position: absolute; right: 52px; top: 48px; width: 132px; height: 132px; border-radius: 66px; background: rgba(14,14,16,0.55); box-shadow: inset 0 0 0 2px rgba(255,255,255,0.8)"><svg width="132" height="132" aria-hidden="true"><polygon points="66,56 73,74 66,70 59,74" fill="#FFFFFF"></polygon><circle cx="98" cy="40" r="6" fill="{CAT['light']}"></circle></svg></div>
<svg width="40" height="40" style="position: absolute; left: 540px; top: 295px" aria-hidden="true"><path d="M20 4 V14 M20 26 V36 M4 20 H14 M26 20 H36" stroke="#FFFFFF" stroke-width="2.5" stroke-linecap="round"></path></svg>
<div style="position: absolute; right: 56px; bottom: 46px; display: flex; align-items: baseline; gap: 6px; color: #FFFFFF"><span style="font: 700 52px/1 'Roboto Mono', monospace">24</span><span style="font: 500 20px/1 'Roboto Mono', monospace; opacity: .75">/ 120</span></div>
</div>'''
    sizebar = (f'<div role="radiogroup" aria-label="Screen shape" style="position: absolute; left: 40px; top: 22px; display: flex; padding: 3px; border-radius: 10px; background: rgba(255,255,255,0.07)">'
               + ''.join(f'<button type="button" role="radio" aria-checked="{str(i == 0).lower()}" style="display: inline-flex; align-items: center; gap: 6px; height: 26px; padding: 0 12px; border-radius: 7px; font-size: 12.5px; '
                         + (f'background: {a}; color: #FFFFFF; font-weight: 500">' if i == 0 else 'color: #B0B0B6">') + ic('window', 'currentColor', 12) + s + '</button>'
                         for i, s in enumerate(['16:9', '21:9', '4:3', 'Phone'])) + '</div>')
    names = ['top left', 'top', 'top right', 'left', 'centre', 'right', 'bottom left', 'bottom', 'bottom right']
    cells = ''.join(f'<button type="button" aria-label="Pin {names[i]}" aria-pressed="{str(i == 0).lower()}" style="width: 24px; height: 24px; display: flex; align-items: center; justify-content: center; border-radius: 6px{"; background: " + hexa(t["acc2"], 0.16) if i == 0 else ""}">'
                    + (f'<span style="width: 10px; height: 10px; border-radius: 3px; background: {t["acc2"]}"></span>' if i == 0 else '<span style="width: 4px; height: 4px; border-radius: 2px; background: #6E6E75"></span>') + '</button>' for i in range(9))
    anchor = (f'<div class="row" style="align-items: flex-start"><span class="lbl" style="padding-top: 4px">Pin to</span>'
              f'<div style="display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 2px; padding: 4px; width: 84px; border-radius: 9px; background: #1F1F23">{cells}</div>'
              f'<span style="font-size: 12.5px; color: #FFFFFF; font-weight: 500; padding-top: 4px">Top left</span></div>')
    ins = (ident(a, 'layout', 'Health bar', 'Widget · Bar')
           + grp('Position', anchor + ws('Margin', ['Snug', 'Comfy', 'Roomy'], 1), 'pin', t['acc2'])
           + grp('Shows', dd('Value', 'Player › Health') + ws('Warn below', ['Never', 'Quarter', 'Half'], 1) + trow(t, 'Animate changes', True), 'eye', CAT['act'])
           + grp('Style', sw('Fill', CAT['flow'], 'Neon green') + seg('Shape', ['Flat', 'Rounded', 'Pill'], 2, True) + ws('Size', ['Small', 'Medium', 'Large'], 1), 'drop', CAT['light'])
           + grp('Advanced', '', 'sliders', '#8E8E95', '3 settings'))
    L = lst([('On screen', [(a, 'Health bar', True, '', 'layout'), (a, 'Objective text', False, '', 'text'), (a, 'Minimap', False, '', 'map'), (a, 'Crosshair', False, '', 'target'), (a, 'Ammo counter', False, '', 'text')]),
             ('Hidden until needed', [('#8E8E95', 'Pause hint', False, '', 'text'), ('#8E8E95', 'Damage flash', False, '', 'spark')])], 'Find widget')
    view = screen + sizebar + notch(t, ['Layout', 'Widgets', 'Fonts', 'Bindings'], 'Layout') + tag(t, 96, 98, 'Health bar', 'layout') + dock(t, ins, L, 'Widgets', 'target')
    write('Hud.dc.html', page('UI/HUD', 'Beverly — UI/HUD', view, DOCK_SCRIPT, [('Move', 'Drag'), ('Nudge', 'Arrows'), ('Zoom', 'Wheel'), ('Test HUD', '`')],
                              world=svg(room(None, a)), view_bg='rgba(14,14,16,0.9)'))


# ---------------- EFFECTS ----------------
def effects():
    t = THEMES['Effects']
    a = t['acc']
    inner = f'''<defs>
<linearGradient id="po" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#FF6A00" stop-opacity=".95"></stop><stop offset="1" stop-color="#FF2D46" stop-opacity="0"></stop></linearGradient>
<linearGradient id="py" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#FFE11A"></stop><stop offset="1" stop-color="#FFB000" stop-opacity="0"></stop></linearGradient>
<linearGradient id="pw" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#FFFFFF"></stop><stop offset="1" stop-color="#FFFFFF" stop-opacity="0"></stop></linearGradient>
<radialGradient id="pg" cx="0.5" cy="0.5" r="0.5"><stop offset="0" stop-color="#FF8A1A" stop-opacity=".55"></stop><stop offset="1" stop-color="#FF8A1A" stop-opacity="0"></stop></radialGradient>
<radialGradient id="dust" cx="0.5" cy="0.5" r="0.5"><stop offset="0" stop-color="#E8D6B0" stop-opacity=".9"></stop><stop offset="1" stop-color="#E8D6B0" stop-opacity="0"></stop></radialGradient>
</defs>
<ellipse cx="620" cy="596" rx="230" ry="40" fill="url(#dust)"></ellipse>
<ellipse cx="620" cy="300" rx="170" ry="170" fill="url(#pg)"></ellipse>
<path d="M578 232 C 552 340, 580 480, 620 572 C 660 480, 688 340, 662 232 Z" fill="url(#po)"></path>
<path d="M594 232 C 582 312, 598 420, 620 486 C 642 420, 658 312, 646 232 Z" fill="url(#py)"></path>
<path d="M605 232 C 600 280, 610 340, 620 384 C 630 340, 640 280, 635 232 Z" fill="url(#pw)"></path>
<polygon points="620,266 628,278 620,290 612,278" fill="#FFFFFF" opacity=".95"></polygon>
<polygon points="620,312 627,322 620,332 613,322" fill="#FFFFFF" opacity=".75"></polygon>
<polygon points="620,354 626,363 620,372 614,363" fill="#FFFFFF" opacity=".55"></polygon>
<polygon points="620,394 625,401 620,408 615,401" fill="#FFFFFF" opacity=".35"></polygon>
<rect x="556" y="102" width="128" height="86" rx="16" fill="#B4B8C0"></rect>
<rect x="556" y="150" width="128" height="14" fill="#8A8E96"></rect>
<polygon points="588,188 652,188 664,234 576,234" fill="#5A5E66"></polygon>
<rect x="572" y="226" width="96" height="10" rx="3" fill="#3E4148"></rect>
<rect x="556" y="226" width="128" height="352" rx="12" fill="none" stroke="{a}" stroke-width="2" stroke-dasharray="7 5"></rect>'''
    ramp = (f'<div class="row" style="align-items: flex-start"><span class="lbl" style="padding-top: 1px">Colour</span><div style="flex: 1">'
            f'<div style="position: relative; height: 16px; border-radius: 8px; background: linear-gradient(90deg, #FFFFFF 0%, #FFE11A 25%, #FF6A00 55%, #FF2D46 80%, rgba(255,45,70,0) 100%), #26262A">'
            + ''.join(f'<span style="position: absolute; left: {p}%; top: 18px; width: 10px; height: 10px; margin-left: -5px; border-radius: 5px; background: {c}; box-shadow: 0 0 0 2px #FFFFFF"></span>' for p, c in ((2, '#FFFFFF'), (25, '#FFE11A'), (55, '#FF6A00'), (80, '#FF2D46')))
            + '</div><div style="display: flex; justify-content: space-between; margin-top: 18px; font-size: 11.5px; color: #9C9CA3"><span>Core</span><span>Tail</span></div></div></div>')
    ins = (ident(a, 'flame', 'Jump jet plume', 'Effect · Plume · on Mech leg')
           + grp('Shape', ws('Length', ['Short', 'Medium', 'Long', 'Blast'], 2, True) + ws('Spread', ['Tight', 'Even', 'Wide'], 0, True) + trow(t, 'Shock diamonds', True), 'cone', CAT['shape'])
           + grp('Heat', ramp + ws('Glow', ['Soft', 'Bright', 'Blinding'], 1), 'heat', CAT['trig'])
           + grp('Motion', ws('Flicker', ['Calm', 'Lively', 'Wild'], 1) + dd('Follows', 'Throttle input', chg=True), 'wind', t['acc2'])
           + grp('Advanced', '', 'sliders', '#8E8E95', '7 settings'))
    L = lst([('Plumes', [(a, 'Jump jet plume', True, 'left leg', 'flame'), (a, 'Jump jet plume', False, 'right leg', 'flame')]),
             ('Particles', [(CAT['light'], 'Dust kick', False, '', 'spark')]), ('Post processing', [(t['acc2'], 'Heat haze', False, '', 'aperture')])], 'Find effect')
    view = (notch(t, ['Plumes', 'Particles', 'Fire', 'Post'], 'Plumes') + status(t, 'Preview · Full throttle', 'play')
            + tag(t, 694, 226, 'Jump jet plume', 'flame') + dock(t, ins, L, 'Effects', 'spark'))
    write('Effects.dc.html', page('Effects', 'Beverly — Effects', view, DOCK_SCRIPT, [('Orbit', 'Middle-drag'), ('Replay', 'R'), ('Throttle', 'Wheel')], world=svg(inner)))


# ---------------- ANIMATICS ----------------
def animatics():
    t = THEMES['Animatics']
    a = t['acc']
    paper = t['bg']
    frame = f'''<div style="position: absolute; left: 340px; top: 62px; width: 840px; height: 472px; border-radius: 12px; overflow: hidden; box-shadow: 0 0 0 1px rgba(255,255,255,0.16), 0 20px 50px rgba(0,0,0,0.45)">
{svg(room(None, a), 'a', '840', '472', '430 200 800 450')}
<div style="position: absolute; left: 50%; bottom: 26px; transform: translateX(-50%); padding: 8px 16px; border-radius: 8px; background: rgba(14,14,16,0.7); font-size: 18px; font-weight: 500; color: #FFFFFF; white-space: nowrap">The door creaks open.</div>
</div>
<div style="position: absolute; left: 340px; top: 30px; display: flex; align-items: center; gap: 10px">{ic('film', a, 15)}<span style="font-size: 16px; font-weight: 700">Shot 2 · Door</span><span style="font: 500 13px 'Roboto Mono', monospace; color: #9C9CA3">2.4 s</span>{ic('camera', CAT['cam'], 14)}<span style="font-size: 13px; color: #B0B0B6">Camera 1</span></div>'''
    names = [(a, 'Shots', '', 'film'), (CAT['cam'], 'Camera 1', 'Move', 'camera'), ('#F3F3F1', 'Captions', '', 'caption'), (CAT['act'], 'Music', '', 'music')]
    lanes = [block(0, 1.6, 0, '#3A3A40', '#F3F3F1', '1  Wide'), block(1.6, 4.0, 0, paper, INK, '2  Door', a),
             block(4.0, 6.2, 0, '#3A3A40', '#F3F3F1', '3  Close-up'), block(6.2, 9.0, 0, '#3A3A40', '#F3F3F1', '4  Reveal'),
             span(0, 9.0, 1, CAT['cam'])] + [key(s, 1, CAT['cam']) for s in (0, 1.6, 4.0, 6.2, 9.0)] + [
             block(1.8, 3.8, 2, 'rgba(255,255,255,0.14)', '#F3F3F1', 'The door creaks open.'),
             block(6.4, 8.8, 2, 'rgba(255,255,255,0.14)', '#F3F3F1', 'Something waits inside.'),
             block(0, 9.0, 3, hexa(CAT['act'], 0.3), '#CFE0FF', 'tension.ogg')]
    view = (frame + notch(t, ['Shots', 'Cameras', 'Captions', 'Sound'], 'Shots') + rail(t)
            + timeline(t, ['Opening', 'Shot 2'], names, lanes, 2.6, 0, addkey('New shot', 'N', 'plus', t)))
    write('Animatics.dc.html', page('Animatics', 'Beverly — Animatics', view, STATIC_SCRIPT, [('Play', 'Enter'), ('Scrub', 'Drag'), ('Next shot', 'Page Down')],
                                    world=svg(room(None, a)), view_bg='rgba(14,14,16,0.9)'))


if __name__ == '__main__':
    level(); level_timeline(); characters(); vehicles(); hud(); effects(); animatics()
    print('ok')


# ---------------- SCREENS ----------------
def screens():
    t = THEMES['Screens']
    a, a2 = t['acc'], t['acc2']
    LEVEL_Y = THEMES['Level']['acc']
    nodes, wires, ports = [], [], {}

    def prev(kind):
        s = '<svg width="100%" height="100%" viewBox="0 0 160 90" preserveAspectRatio="xMidYMid meet" aria-hidden="true">'
        if kind == 'title':
            s += f'<rect x="45" y="30" width="70" height="14" rx="3" fill="#F3F3F1"></rect><rect x="58" y="56" width="44" height="6" rx="3" fill="#F3F3F1" opacity=".45"></rect>'
        elif kind == 'main':
            s += f'<rect x="16" y="14" width="64" height="10" rx="2" fill="#F3F3F1"></rect>'
            for i in range(4):
                s += f'<rect x="16" y="{34+i*12}" width="{54 if i else 58}" height="8" rx="4" fill="{a if i == 0 else "#F3F3F1"}" opacity="{1 if i == 0 else .45}"></rect>'
        elif kind == 'options':
            s += f'<rect x="16" y="12" width="50" height="8" rx="2" fill="#F3F3F1"></rect>'
            for i in range(3):
                s += f'<rect x="16" y="{32+i*16}" width="34" height="6" rx="3" fill="#F3F3F1" opacity=".45"></rect><rect x="70" y="{33+i*16}" width="74" height="4" rx="2" fill="#F3F3F1" opacity=".25"></rect><rect x="70" y="{33+i*16}" width="{30+i*14}" height="4" rx="2" fill="{a2}"></rect>'
        elif kind == 'credits':
            for i, w in enumerate((70, 50, 60, 40, 54)):
                s += f'<rect x="{80-w/2}" y="{14+i*13}" width="{w}" height="6" rx="3" fill="#F3F3F1" opacity="{.8 if i == 0 else .4}"></rect>'
        elif kind == 'quit':
            s += f'<rect x="0" y="0" width="160" height="90" fill="#000000" opacity=".35"></rect><rect x="38" y="20" width="84" height="50" rx="6" fill="#26262B"></rect><rect x="50" y="30" width="60" height="6" rx="3" fill="#F3F3F1" opacity=".8"></rect><rect x="50" y="50" width="26" height="9" rx="4.5" fill="{a}"></rect><rect x="84" y="50" width="26" height="9" rx="4.5" fill="#F3F3F1" opacity=".45"></rect>'
        return s + '</svg>'

    def node(key, x, y, w, title, icon, kind, rows, sel_node=False, sel_row=None, badge='', warn=''):
        ph = round((w - 24) * 9 / 16)
        top = y + 36 + ph + 8
        ports[key + ':in'] = (x, y + 18)
        rws = []
        for i, (label, ricon) in enumerate(rows):
            cy = top + i * 28 + 14
            ports[f'{key}:{label}'] = (x + w, cy)
            on = i == sel_row
            st = f'background: {hexa(a, 0.2)}; color: #FFFFFF; font-weight: 500;' if on else 'color: #D2D2D6;'
            rws.append(f'<div style="display: flex; align-items: center; gap: 8px; height: 28px; margin: 0 6px; padding: 0 8px; border-radius: 7px; font-size: 12.5px; {st}">{ic(ricon, a if on else "#8E8E95", 14)}{label}</div>')
        ring = f'box-shadow: 0 0 0 2px {a}, 0 12px 30px rgba(0,0,0,0.45);' if sel_node else 'box-shadow: inset 0 0 0 1px rgba(255,255,255,0.1), 0 10px 24px rgba(0,0,0,0.35);'
        b = f'<span style="margin-left: auto; display: inline-flex; align-items: center; gap: 4px; height: 20px; padding: 0 7px; border-radius: 10px; background: {hexa(a2, 0.18)}; color: {a2}; font-size: 11px; font-weight: 500">{ic("home", a2, 12)}{badge}</span>' if badge else ''
        wn = f'<div style="display: flex; align-items: center; gap: 7px; margin: 6px 6px 0; padding: 0 8px; height: 28px; border-radius: 7px; background: {hexa(DANGER, 0.16)}; color: #FF8A98; font-size: 12px; font-weight: 500">{ic("alert", DANGER, 14)}{warn}</div>' if warn else ''
        dots = ''.join(f'<span style="position: absolute; right: -6px; top: {36 + ph + 8 + i*28 + 8}px; width: 10px; height: 10px; border-radius: 5px; background: {a if i == sel_row else "#3A3A40"}; box-shadow: 0 0 0 2px #17171A"></span>' for i in range(len(rows)))
        nodes.append(f'''<div style="position: absolute; left: {x}px; top: {y}px; width: {w}px; padding-bottom: 10px; border-radius: 14px; background: #17171A; {ring}">
<div style="display: flex; align-items: center; gap: 8px; height: 36px; padding: 0 10px 0 12px">{ic(icon, a, 16)}<span style="font-size: 13px; font-weight: 700">{title}</span>{b}</div>
<div style="margin: 0 12px; height: {ph}px; border-radius: 8px; background: #0B0B0D; overflow: hidden">{prev(kind)}</div>
<div style="height: 8px"></div>{"".join(rws)}{wn}
<span style="position: absolute; left: -6px; top: 13px; width: 10px; height: 10px; border-radius: 5px; background: #6E6E78; box-shadow: 0 0 0 2px #17171A"></span>{dots}
</div>''')

    def exitnode(key, x, y, title, sub, icon, color):
        ports[key + ':in'] = (x, y + 32)
        nodes.append(f'''<div style="position: absolute; left: {x}px; top: {y}px; width: 200px; height: 64px; display: flex; align-items: center; gap: 12px; padding: 0 14px; box-sizing: border-box; border-radius: 32px; background: #17171A; box-shadow: inset 0 0 0 1.5px {hexa(color, 0.6)}">
<span style="width: 34px; height: 34px; border-radius: 17px; background: {hexa(color, 0.18)}; display: flex; align-items: center; justify-content: center">{ic(icon, color, 18)}</span>
<span style="display: flex; flex-direction: column; gap: 3px"><span style="font-size: 13px; font-weight: 700">{title}</span><span style="font-size: 11.5px; color: #9C9CA3">{sub}</span></span>
<span style="position: absolute; left: -6px; top: 27px; width: 10px; height: 10px; border-radius: 5px; background: #6E6E78; box-shadow: 0 0 0 2px #17171A"></span></div>''')

    node('title', 40, 270, 208, 'Title', 'monitor', 'title', [('Any key', 'keyboard')], badge='Start')
    node('main', 320, 170, 232, 'Main menu', 'monitor', 'main', [('Play', 'play'), ('Options', 'sliders'), ('Credits', 'text'), ('Quit', 'x')], True, 0)
    node('opt', 650, 60, 208, 'Options', 'monitor', 'options', [('Sound', 'music'), ('Controls', 'keyboard'), ('Back', 'back')])
    node('cred', 650, 352, 208, 'Credits', 'monitor', 'credits', [], warn='No way back')
    node('quit', 650, 580, 208, 'Quit?', 'layers', 'quit', [('Yes', 'check'), ('No', 'back')])
    exitnode('lvl', 960, 220, 'Level 1', 'Starts the game', 'mountain', LEVEL_Y)
    exitnode('close', 960, 620, 'Close game', 'Back to desktop', 'x', '#B0B0B6')

    def fwd(src, dst, color='#6E6E78', w=2, dash=''):
        (x1, y1), (x2, y2) = ports[src], ports[dst]
        d = f' stroke-dasharray="{dash}"' if dash else ''
        wires.append(f'<path d="M{x1+5} {y1} C {x1+80} {y1}, {x2-80} {y2}, {x2-5} {y2}" fill="none" stroke="{color}" stroke-width="{w}" stroke-linecap="round"{d}></path>')

    def back(src, dst, ylane, xlane=292):
        (x1, y1), (x2, y2) = ports[src], ports[dst]
        r = 16
        up = ylane < y1
        s = -1 if up else 1
        wires.append(f'<path d="M{x1+5} {y1} H{x1+24} Q{x1+40} {y1} {x1+40} {y1 + s*r} V{ylane - s*r} Q{x1+40} {ylane} {x1+24} {ylane} H{xlane+r} Q{xlane} {ylane} {xlane} {ylane - s*r if False else ylane + (-s)*r} V{y2 + s*r} Q{xlane} {y2} {xlane+r} {y2} H{x2-5}" fill="none" stroke="#6E6E78" stroke-width="2" stroke-dasharray="6 5" stroke-linecap="round"></path>')

    fwd('title:Any key', 'main:in')
    fwd('main:Play', 'lvl:in', a, 3)
    fwd('main:Options', 'opt:in'); fwd('main:Credits', 'cred:in'); fwd('main:Quit', 'quit:in')
    fwd('quit:Yes', 'close:in')
    back('opt:Back', 'main:in', 40)
    back('quit:No', 'main:in', 846)

    wire_svg = f'<svg width="1584" height="908" style="position: absolute; inset: 0" aria-hidden="true">{"".join(wires)}</svg>'
    chip = (f'<div class="pill" style="position: absolute; left: 700px; top: 238px; height: 26px; padding: 0 10px; font-size: 11.5px">{ic("swap", a2, 13)}Fade · Smooth</div>')
    check = f'''<section aria-label="Flow check" class="glass" style="position: absolute; left: 16px; bottom: 16px; width: 256px; padding: 12px 14px; border-radius: 14px; display: flex; flex-direction: column; gap: 9px; font-size: 12.5px">
<div style="display: flex; align-items: center; gap: 8px; font-weight: 700">{ic('route', a2, 16)}Flow check</div>
<div style="display: flex; align-items: center; gap: 8px; color: #D2D2D6">{ic('check', CAT['flow'], 14, 2.5)}Every screen can be reached</div>
<div style="display: flex; align-items: center; gap: 8px; color: #D2D2D6">{ic('check', CAT['flow'], 14, 2.5)}Deepest screen: 2 clicks</div>
<div style="display: flex; align-items: center; gap: 8px; color: #FF8A98">{ic('alert', DANGER, 14)}Credits has no way back<button type="button" style="margin-left: auto; height: 22px; padding: 0 9px; border-radius: 6px; background: #2A2A2F; color: #F3F3F1; font-size: 11.5px; font-weight: 500">Add Back</button></div>
</section>'''
    canvas = (f'<div style="position: absolute; inset: 0; background-image: radial-gradient(rgba(255,255,255,0.07) 1px, transparent 1.2px); background-size: 24px 24px"></div>'
              + wire_svg + ''.join(nodes) + chip + check)

    ins = (ident(a, 'pointer', 'Play', 'Button · on Main menu')
           + grp('Goes to', dd('Opens', 'Level 1', ic('mountain', LEVEL_Y, 16)) + seg('Then', ['Replace', 'On top'], 0), 'route', a2)
           + grp('Transition', seg('Style', ['Cut', 'Fade', 'Slide', 'Wipe'], 1, True) + ws('Speed', ['Snappy', 'Smooth', 'Slow'], 1) + dd('Sound', 'UI confirm', ic('music', CAT['act'], 16)), 'swap', CAT['act'])
           + grp('Looks', dd('Style', 'Primary button') + ws('Size', ['Small', 'Medium', 'Large'], 2, True) + trow(t, 'Focused first', True, 'Gamepad'), 'palette', CAT['light'])
           + grp('Advanced', '', 'sliders', '#8E8E95', '4 settings'))
    L = lst([('Screens', [(a, 'Title', False, 'Start', 'monitor'), (a, 'Main menu', True, '', 'monitor'), (a, 'Options', False, '', 'monitor'), (a, 'Credits', False, '!No way back', 'monitor')]),
             ('Pop-ups', [(a, 'Quit?', False, '', 'layers')]),
             ('Leads out to', [(LEVEL_Y, 'Level 1', False, 'Level tab', 'mountain'), ('#B0B0B6', 'Close game', False, '', 'x')])], 'Find screen')
    view = (canvas + notch(t, ['Menus', 'Flow', 'Transitions', 'Styles'], 'Flow') + dock(t, ins, L, 'Screens', 'monitor'))
    write('Screens.dc.html', page('Screens', 'Beverly — Screens', view, DOCK_SCRIPT, [('Pan', 'Space'), ('Connect', 'Drag a port'), ('Zoom', 'Wheel'), ('Test menus', '`')],
                                  world=svg(room(None, a)), view_bg='rgba(14,14,16,0.92)'))


if __name__ == '__main__':
    screens()
