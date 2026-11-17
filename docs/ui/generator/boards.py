import os
import sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from base import *
from icons import ic, GROUPS

OUT = os.path.join(os.path.dirname(os.path.abspath(__file__)), 'out') + os.sep
os.makedirs(OUT, exist_ok=True)


def shell(title, w, h, body):
    return f'''<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>{title}</title>
<script src="./support.js"></script>
</head>
<body>
<x-dc>
<helmet>
<link href="https://fonts.googleapis.com/css2?family=Roboto:wght@400;500;700&amp;family=Roboto+Mono:wght@400;500&amp;display=swap" rel="stylesheet">
<style>
body{{margin:0}}
</style>
</helmet>
<div style="width: {w}px; height: {h}px; box-sizing: border-box; padding: 56px; background: {INK}; font-family: Roboto, sans-serif; color: {TXT}; display: flex; flex-direction: column; gap: 28px">
{body}
</div>
</x-dc>
<script type="text/x-dc" data-dc-script data-props='{{"$preview":{{"width":{w},"height":{h}}}}}'>
class Component extends DCLogic {{
  renderVals() {{ return {{}}; }}
}}
</script>
</body>
</html>
'''


def chip(c, name, icon=None):
    i = ic(icon, c, 18) if icon else f'<span style="width: 14px; height: 14px; border-radius: 4px; background: {c}"></span>'
    return f'<div style="display: flex; align-items: center; gap: 10px; height: 40px; padding: 0 14px; border-radius: 10px; background: #1A1A1D; font-size: 14px">{i}{name}<span style="font: 13px \'Roboto Mono\', monospace; color: #9C9CA3">{c}</span></div>'


def tabs_board():
    def card(n):
        T = THEMES[n]
        fg = T['fg']
        rows = [T['bg']] + ([T['acc']] if T['acc'] != T['bg'] else []) + [T['acc2']]
        sw = ''.join(f'<div style="display: flex; align-items: center; gap: 8px; font: 500 12.5px \'Roboto Mono\', monospace"><span style="width: 14px; height: 14px; border-radius: 4px; background: {c}; box-shadow: inset 0 0 0 1px rgba(128,128,128,0.4)"></span>{c}</div>' for c in rows)
        border = 'box-shadow: inset 0 0 0 1px rgba(255,255,255,0.18);' if n == 'Level' else ''
        return f'''<div style="flex: 1; height: 260px; border-radius: 16px; background: {T['bg']}; color: {fg}; padding: 18px; box-sizing: border-box; display: flex; flex-direction: column; justify-content: space-between; {border}">
<div style="display: flex; align-items: center; gap: 8px; font-size: 20px; font-weight: 700; letter-spacing: -0.01em">{ic(TAB_ICON[n], fg, 20)}{n}</div>
<div style="height: 36px; border-radius: 9px; background: {T['acc2']}"></div>
<div style="display: flex; flex-direction: column; gap: 6px">{sw}</div></div>'''
    cats = ''.join(chip(CAT[k], n, i) for k, n, i in [('shape', 'Shapes', 'cube'), ('trig', 'Triggers', 'bolt'), ('act', 'Actions', 'play'), ('flow', 'Flow', 'route'),
                                                       ('logic', 'Logic', 'diamond'), ('game', 'Game', 'person'), ('light', 'Lights', 'bulb'), ('cam', 'Cameras', 'camera')])
    axes = ''.join(chip(c, n) for c, n in zip(AX, ['X', 'Y', 'Z'])) + chip(DANGER, 'Danger', 'alert')
    body = f'''<div style="display: flex; align-items: baseline; gap: 20px"><h1 style="margin: 0; font-size: 32px; font-weight: 700; letter-spacing: -0.02em">Tab colours</h1><span style="font-size: 15px; color: #B0B0B6">Frame colour marks where you are. The complementary accent marks values you can change.</span></div>
<div style="display: flex; gap: 12px">{''.join(card(n) for n in TAB_ORDER)}</div>
<div style="display: flex; flex-direction: column; gap: 12px">
<div style="font-size: 14px; color: #B0B0B6">Part categories and axes — the same in every tab</div>
<div style="display: flex; flex-wrap: wrap; gap: 8px">{cats}</div>
<div style="display: flex; flex-wrap: wrap; gap: 8px">{axes}</div>
</div>'''
    open(OUT + 'Tabs.dc.html', 'w').write(shell('Beverly — tab colours', 1600, 680, body))


NAMES = dict(cube='Shape', bolt='Trigger', play='Action', route='Flow', diamond='Logic · Key', person='Game · Character', bulb='Light', camera='Camera',
             mountain='Level', car='Vehicle', layout='UI/HUD', wave='Audio', flame='Effect', monitor='Screen', film='Animatic',
             grid='All · Snap', feel='Physics', drop='Look · Water', leaf='Foliage', globe='World', run='Moves', keyboard='Controls', wheel='Drive',
             seat='Seats', target='Widget', text='Text', wire='Wire', spark='Particles', heat='Heat · Fire', aperture='Post', caption='Caption',
             music='Sound', swap='Transition', palette='Style', move='Placement', star='Ability', pin='Pin to', eye='Shows', gauge='Handling',
             cone='Shape', wind='Motion', map='Minimap', home='Start', pointer='Button',
             sliders='Inspect', layers='Scene · Pop-up', folder='Assets', search='Find', clock='Timeline', undo='Undo', redo='Redo',
             left='Open', right='Collapse', down='Expanded', copy='Duplicate', trash='Delete', reset='Reset', plus='Add', x='Close',
             check='Done', alert='Warning', loop='Loop', start='To start', back='Back')
TINT = dict(cube=CAT['shape'], bolt=CAT['trig'], play=CAT['act'], route=CAT['flow'], diamond=CAT['logic'], person=CAT['game'], bulb=CAT['light'], camera=CAT['cam'])


def icons_board():
    secs = []
    for title, names in GROUPS:
        if title == 'Tabs':
            cells = ''.join(f'<div style="display: flex; flex-direction: column; align-items: center; gap: 10px; width: 120px; padding: 16px 0; border-radius: 12px; background: #17171A">'
                            f'<span style="width: 44px; height: 44px; border-radius: 12px; background: {THEMES[n]["bg"]}; color: {THEMES[n]["fg"]}; display: flex; align-items: center; justify-content: center; box-shadow: inset 0 0 0 1px rgba(255,255,255,0.15)">{ic(TAB_ICON[n], "currentColor", 24)}</span>'
                            f'<span style="font-size: 13px; color: #D2D2D6">{n}</span></div>' for n in TAB_ORDER)
        else:
            cells = ''.join(f'<div style="display: flex; flex-direction: column; align-items: center; gap: 10px; width: 120px; padding: 16px 0; border-radius: 12px; background: #17171A">'
                            f'{ic(n, TINT.get(n, "#E6E6E8"), 24)}<span style="font-size: 13px; color: #D2D2D6; text-align: center">{NAMES.get(n, n)}</span></div>' for n in names)
        secs.append(f'<section style="display: flex; flex-direction: column; gap: 12px"><h2 style="margin: 0; font-size: 15px; font-weight: 500; color: #B0B0B6">{title}</h2><div style="display: flex; flex-wrap: wrap; gap: 8px">{cells}</div></section>')
    sizes = ''.join(f'<div style="display: flex; align-items: center; gap: 10px; font-size: 13px; color: #B0B0B6">{ic("bulb", CAT["light"], s)}{s} px</div>' for s in (14, 16, 20, 24))
    rules = ('<div style="display: flex; gap: 32px; align-items: center; font-size: 14px; color: #D2D2D6">'
             '<span>24 × 24 grid</span><span>2 px stroke</span><span>Round caps and joins</span><span>2 px clear edge</span>'
             f'<span>Outline only — no filled icons</span><span style="margin-left: auto; display: flex; gap: 24px">{sizes}</span></div>')
    body = (f'<div style="display: flex; align-items: baseline; gap: 20px"><h1 style="margin: 0; font-size: 32px; font-weight: 700; letter-spacing: -0.02em">Icons</h1>'
            f'<span style="font-size: 15px; color: #B0B0B6">One family. Part icons carry their category colour; everything else takes the text colour.</span></div>'
            + rules + ''.join(secs))
    open(OUT + 'Icons.dc.html', 'w').write(shell('Beverly — icons', 1600, 1180, body))


if __name__ == '__main__':
    tabs_board(); icons_board(); print('ok')
