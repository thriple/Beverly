from icons import ic
INK = '#0E0E10'
FIELD = '#1F1F23'
LINE = '#2E2E33'
TXT = '#F3F3F1'
TXT2 = '#B0B0B6'
TXT3 = '#8E8E95'
DANGER = '#FF2D46'
CAT = dict(shape='#D6D9E0', trig='#FF7A1A', act='#3D7BFF', flow='#2BF06A', logic='#B066FF',
           game='#FF3D9A', light='#FFE11A', cam='#22E5FF')
AX = ('#FF3B4E', '#39F26A', '#3D7BFF')

THEMES = {
    'Level':      dict(bg='#0E0E10', fg='#F3F3F1', acc='#FFD60A', accFg=INK, acc2='#8C7DFF', btn='#FFD60A', btnFg=INK, alpha=0.78),
    'Characters': dict(bg='#FF6EB4', fg=INK, acc='#FF6EB4', accFg=INK, acc2='#2BF0C8'),
    'Vehicles':   dict(bg='#C6FF1A', fg=INK, acc='#C6FF1A', accFg=INK, acc2='#B34DFF'),
    'UI/HUD':     dict(bg='#2E4BF5', fg='#FFFFFF', acc='#5C78FF', accFg='#FFFFFF', acc2='#FFB020'),
    'Audio':      dict(bg='#19E0FF', fg=INK, acc='#19E0FF', accFg=INK, acc2='#FF7A59'),
    'Effects':    dict(bg='#FF8A1F', fg=INK, acc='#FF8A1F', accFg=INK, acc2='#1EA7FF'),
    'Screens':    dict(bg='#B77BFF', fg=INK, acc='#B77BFF', accFg=INK, acc2='#D8FF4A'),
    'Animatics':  dict(bg='#E9E7E2', fg=INK, acc='#FF2D46', accFg=INK, acc2='#19D3C5'),
}
TAB_FILES = {'Level': 'Main.dc.html', 'Characters': 'Characters.dc.html', 'Vehicles': 'Vehicles.dc.html', 'UI/HUD': 'Hud.dc.html', 'Audio': 'Audio.dc.html',
             'Effects': 'Effects.dc.html', 'Screens': 'Screens.dc.html', 'Animatics': 'Animatics.dc.html'}
TAB_ICON = {'Level': 'mountain', 'Characters': 'person', 'Vehicles': 'car', 'UI/HUD': 'layout', 'Audio': 'wave',
            'Effects': 'flame', 'Screens': 'monitor', 'Animatics': 'film'}
TAB_ORDER = ['Level', 'Characters', 'Vehicles', 'UI/HUD', 'Audio', 'Effects', 'Screens', 'Animatics']


def muted(fg):
    return 'rgba(14,14,16,0.8)' if fg == INK else 'rgba(255,255,255,0.8)'


CSS = '''body{margin:0}
a{color:inherit}
button{font:inherit;color:inherit;background:none;border:0;padding:0;margin:0;cursor:pointer}
input{font:inherit}
h3{margin:0}
.kc{display:inline-flex;align-items:center;height:17px;padding:0 5px;border-radius:4px;background:#26262A;border:1px solid #3A3A40;border-bottom-width:2px;font:500 10.5px/1 Roboto,sans-serif;color:#E6E6E8;white-space:nowrap}
.kf{display:inline-flex;align-items:center;height:18px;padding:0 6px;border-radius:5px;background:@FG@;font:500 10.5px/1 Roboto,sans-serif;color:@BG@;white-space:nowrap}
.mb{display:inline-flex;align-items:center;height:28px;padding:0 9px;border-radius:7px;font-size:12.5px;color:@FG@}
.mb:hover{background:@HOV@}
.wc{display:inline-flex;align-items:center;justify-content:center;width:44px;height:44px;color:@FG@}
.wc:hover{background:@HOV@}
.wc:last-child:hover{background:#E81123;color:#FFFFFF}
.tab{display:inline-flex;align-items:center;gap:6px;height:30px;padding:0 11px;border-radius:8px;font-size:13px;color:@MUT@;text-decoration:none}
.tab:hover{color:@FG@}
.chip{display:inline-flex;align-items:center;gap:6px;height:24px;padding:0 11px;border-radius:12px;font-size:12px;color:@MUT@}
.ib{display:inline-flex;align-items:center;justify-content:center;gap:6px;height:28px;min-width:28px;padding:0 6px;border-radius:7px;color:#D2D2D6;font-size:12px;box-sizing:border-box}
.ib:hover{background:#26262A}
.pill{display:flex;align-items:center;gap:8px;height:28px;padding:0 12px;border-radius:14px;background:rgba(14,14,16,0.72);box-shadow:inset 0 0 0 1px rgba(255,255,255,0.1);font-size:12px;color:#F3F3F1}
.glass{background:rgba(18,18,21,0.84);box-shadow:inset 0 0 0 1px rgba(255,255,255,0.1)}
.grp{border-top:1px solid rgba(255,255,255,0.07);padding:16px 18px 18px}
.gh{display:flex;align-items:center;gap:8px;margin-bottom:14px;font:500 12.5px/1 Roboto,sans-serif;color:#D2D2D6;letter-spacing:.01em}
.row{display:flex;align-items:center;gap:10px;min-height:28px}
.row+.row{margin-top:8px}
.lbl{width:76px;flex:none;font-size:12.5px;color:#B0B0B6}
.lbl2{flex:1;font-size:12.5px;color:#E6E6E8}
.fld{display:flex;align-items:center;gap:8px;flex:1;height:28px;padding:0 10px;border-radius:7px;background:#1F1F23;font-size:12.5px;color:#F3F3F1;box-sizing:border-box}
.ax{display:flex;align-items:center;flex:1;height:26px;border-radius:6px;background:#1F1F23;overflow:hidden;font:400 12px/1 'Roboto Mono',monospace;color:#F3F3F1}
.ax b{width:18px;height:100%;display:flex;align-items:center;justify-content:center;font:700 10px/1 Roboto,sans-serif;color:#0E0E10}
.ax span{padding:0 7px}
.wsr{align-items:flex-start}
.wsr .lbl{padding-top:3px}
.ws{flex:1;padding:0 6px}
.trk{position:relative;height:4px;border-radius:2px;background:#34343A;margin-top:8px}
.fil{position:absolute;left:0;top:0;bottom:0;border-radius:2px;background:@ACC2@}
.tk{position:absolute;top:-2px;width:2px;height:8px;margin-left:-1px;border-radius:1px;background:#4A4A52}
.th{position:absolute;top:-6px;width:16px;height:16px;margin-left:-8px;border-radius:8px;background:#FFFFFF;box-shadow:0 0 0 3px @ACC2A@}
.stp{position:relative;height:14px;margin-top:10px;font-size:11px;color:#8E8E95;white-space:nowrap}
.stp span{position:absolute;top:0}
.stp .on{color:#F3F3F1;font-weight:500}
.tg{position:relative;width:30px;height:16px;border-radius:8px;flex:none}
.tg i{position:absolute;top:2px;width:12px;height:12px;border-radius:6px}
.sg{display:flex;flex:1;padding:2px;border-radius:8px;background:#1F1F23}
.sg span{flex:1;display:flex;align-items:center;justify-content:center;height:24px;border-radius:6px;font-size:12px;color:#B0B0B6}
.sg .on{background:@ACC@;color:@ACCFG@;font-weight:500}
.srow{display:flex;align-items:center;gap:10px;width:100%;height:30px;padding:0 10px;border-radius:7px;font-size:12.5px;color:#DCDCE0;text-align:left;box-sizing:border-box}
.srow:hover{background:#1F1F23}
.dot{width:8px;height:8px;border-radius:2px;flex:none}
.sh{padding:12px 10px 4px;font-size:11.5px;color:#8E8E95}
.fld:hover,.ax:hover{background:#26262B}
.ax{cursor:ew-resize}
.sg span:not(.on):hover{color:#F3F3F1;background:rgba(255,255,255,0.05)}
button:focus-visible,a:focus-visible,.fld:focus-visible{outline:2px solid @ACC2@;outline-offset:2px}
input:focus-visible{outline:none}
.inp{display:flex;align-items:center;gap:8px;height:30px;padding:0 10px;border-radius:7px;background:#1F1F23;box-shadow:inset 0 0 0 1px transparent;font-size:12.5px;color:#F3F3F1;box-sizing:border-box}
.inp:focus-within{box-shadow:inset 0 0 0 1.5px @ACC2@;background:#17171A}
.inp input{flex:1;min-width:0;background:none;border:0;outline:0;color:#F3F3F1;font:inherit}
.btn{display:inline-flex;align-items:center;justify-content:center;gap:7px;height:30px;padding:0 12px;border-radius:8px;font-size:12.5px;font-weight:500;box-sizing:border-box}
.tip{display:inline-flex;align-items:center;gap:8px;height:28px;padding:0 10px;border-radius:8px;background:#F3F3F1;color:#0E0E10;font-size:12px;font-weight:500;box-shadow:0 6px 18px rgba(0,0,0,.4)}
.tip .kc{background:#0E0E10;border-color:#0E0E10;color:#F3F3F1}
.chg{position:relative}
.chg::before{content:'';position:absolute;left:-18px;top:5px;bottom:5px;width:3px;border-radius:0 2px 2px 0;background:@ACC2@}
.chg .lbl,.chg .lbl2{color:#FFFFFF;font-weight:500}
.gh .cv{margin-left:auto;color:#6E6E75;display:flex}
'''


def hexa(h, a):
    return 'rgba(%d,%d,%d,%s)' % (int(h[1:3], 16), int(h[3:5], 16), int(h[5:7], 16), a)


def css(t):
    hov = 'rgba(14,14,16,0.1)' if t['fg'] == INK else 'rgba(255,255,255,0.14)'
    return (CSS.replace('@FG@', t['fg']).replace('@BG@', t['bg']).replace('@MUT@', muted(t['fg']))
            .replace('@HOV@', hov).replace('@ACCFG@', t['accFg']).replace('@ACCA@', hexa(t['acc'], 0.3))
            .replace('@ACC2A@', hexa(t['acc2'], 0.3)).replace('@ACC2@', t['acc2']).replace('@ACC@', t['acc']))


# ---------- small controls ----------
def kc(k):
    return f'<span class="kc">{k}</span>' if k else ''


def tg(t, on):
    if on:
        return f'<span class="tg" style="background: {t["acc2"]}"><i style="left: 16px; background: {INK}"></i></span>'
    return '<span class="tg" style="background: #3A3A40"><i style="left: 2px; background: #9A9AA0"></i></span>'


def lic(icon):
    """A row's label icon: 14 px, in the label's colour, before its word."""
    return ic(icon, 'currentColor', 14).replace('style="flex: none"', 'style="flex: none; vertical-align: -2px; margin-right: 7px"') if icon else ''


def more_btn(acc2, open=False):
    """More: at the right of the row under a slider, its caret turned down while its numbers are open."""
    turn = ' transform: rotate(90deg);' if open else ''
    caret = ic('right', acc2, 12).replace('style="flex: none"', f'style="flex: none;{turn}"')
    return (f'<div style="display: flex; justify-content: flex-end; margin-top: 4px"><button type="button" aria-expanded="{str(open).lower()}" title="Its numbers, and what the slider&apos;s ends mean" '
            f'style="display: inline-flex; align-items: center; gap: 4px; height: 20px; font-size: 12px; font-weight: 500; color: {acc2}">More{caret}</button></div>')


def ws(label, stops, idx, chg=False, icon=None):
    n = len(stops)
    pct = idx / (n - 1) * 100
    ticks = ''.join(f'<span class="tk" style="left: {i/(n-1)*100:.1f}%"></span>' for i in range(n))
    labs = []
    for i, s in enumerate(stops):
        cls = ' class="on"' if i == idx else ''
        if i == 0:
            pos = 'left: 0'
        elif i == n - 1:
            pos = 'right: 0'
        else:
            pos = f'left: {i/(n-1)*100:.1f}%; transform: translateX(-50%)'
        labs.append(f'<span{cls} style="{pos}">{s}</span>')
    c = ' chg' if chg else ''
    return (f'<div class="row wsr{c}"{CHG_T if chg else ""}><span class="lbl">{lic(icon)}{label}</span><div class="ws"><div class="trk">'
            f'<div class="fil" style="width: {pct:.1f}%"></div>{ticks}<span class="th" style="left: {pct:.1f}%"></span></div>'
            f'<div class="stp">{"".join(labs)}</div></div></div>')


def trow(t, label, on, key=None, chg=False, icon=None):
    return f'<div class="row{" chg" if chg else ""}"{CHG_T if chg else ""}><span class="lbl2">{lic(icon)}{label}</span>{kc(key)}{tg(t, on)}</div>'


def seg(label, opts, idx, chg=False, icon=None):
    s = ''.join(f'<span class="on">{o}</span>' if i == idx else f'<span>{o}</span>' for i, o in enumerate(opts))
    return f'<div class="row{" chg" if chg else ""}"{CHG_T if chg else ""}><span class="lbl">{lic(icon)}{label}</span><div class="sg" role="radiogroup" aria-label="{label}">{s}</div></div>'


CHG_T = ' title="Changed from default. Click the bar to reset."'
CHEV = '<svg style="margin-left: auto" width="10" height="10" viewBox="0 0 10 10" fill="none" stroke="#8E8E95" stroke-width="1.5" aria-hidden="true"><path d="M2 3.5 L5 6.5 L8 3.5"></path></svg>'


def dd(label, value, lead='', chg=False):
    return f'<div class="row{" chg" if chg else ""}"{CHG_T if chg else ""}><span class="lbl">{label}</span><button type="button" class="fld">{lead}{value}{CHEV}</button></div>'


def sw(label, color, name):
    return dd(label, name, f'<span style="width: 18px; height: 18px; border-radius: 5px; background: {color}; box-shadow: inset 0 0 0 1px rgba(255,255,255,0.2); margin-left: -4px"></span>')


def axrow(label, vals):
    cells = ''.join(f'<div class="ax"><b style="background: {AX[i]}">{"XYZ"[i]}</b><span>{v}</span></div>' for i, v in enumerate(vals))
    return f'<div class="row"><span class="lbl">{label}</span>{cells}</div>'


def grp(title, inner, icon=None, color='#B0B0B6', collapsed=None):
    i = ic(icon, color, 16) if icon else ''
    if collapsed:
        return (f'<section class="grp" style="padding-bottom: 16px"><button type="button" class="gh" aria-expanded="false" style="width: 100%; margin: 0">{i}{title}'
                f'<span style="font-size: 12px; color: #8E8E95; margin-left: 4px">{collapsed}</span><span class="cv">{ic("right", "currentColor", 14)}</span></button></section>')
    return f'<section class="grp"><h3 class="gh">{i}{title}<span class="cv">{ic("down", "currentColor", 14)}</span></h3>{inner}</section>'


def ident(color, icon, title, sub, actions=True):
    act = ''
    if actions:
        act = (f'<button type="button" class="ib" aria-label="Duplicate" title="Duplicate  Ctrl D">{ic("copy", "currentColor", 16)}</button>'
               f'<button type="button" class="ib" aria-label="Delete" title="Delete  Del">{ic("trash", "currentColor", 16)}</button>')
    return (f'<div style="display: flex; align-items: center; gap: 12px; padding: 6px 18px 16px">'
            f'<div style="width: 40px; height: 40px; border-radius: 11px; background: {hexa(color, 0.16)}; box-shadow: inset 0 0 0 1px {hexa(color, 0.35)}; display: flex; align-items: center; justify-content: center; flex: none">'
            f'{ic(icon, color, 20)}</div>'
            f'<div style="flex: 1; min-width: 0"><div style="font-size: 16px; font-weight: 700; letter-spacing: -0.01em">{title}</div>'
            f'<div style="font-size: 12px; color: #A0A0A7; margin-top: 3px">{sub}</div></div>{act}</div>')


def lst(groups, search='Find'):
    out = [f'<label style="display: flex; align-items: center; gap: 8px; height: 30px; margin: 0 2px 6px; padding: 0 10px; border-radius: 8px; background: #1F1F23; color: #8E8E95; font-size: 12.5px">'
           + ic('search', 'currentColor', 14) +
           f'<input type="text" placeholder="{search}" aria-label="{search}" style="flex: 1; min-width: 0; background: none; border: 0; outline: 0; color: #F3F3F1; font-size: 12.5px"></label>']
    for head, rows in groups:
        out.append(f'<div class="sh">{head}</div>')
        for r in rows:
            color, name, sel, note = r[:4]
            icon = r[4] if len(r) > 4 else 'cube'
            st = ' style="background: #2A2A2F; color: #FFFFFF"' if sel else ''
            nt = f'<span style="margin-left: auto; font-size: 11.5px; color: {DANGER if note and note.startswith("!") else "#8E8E95"}">{note.lstrip("!")}</span>' if note else ''
            out.append(f'<button class="srow" type="button"{st}>{ic(icon, color, 14)}{name}{nt}</button>')
    return '<div style="flex: 1; overflow: auto; padding: 0 8px 12px">' + '\n'.join(out) + '</div>'


TRI = '<svg width="10" height="10" viewBox="0 0 10 10" fill="#8E8E95" aria-hidden="true"><path d="M3 1.5 L7.5 5 L3 8.5 Z"></path></svg>'
TRID = '<svg width="10" height="10" viewBox="0 0 10 10" fill="#F3F3F1" aria-hidden="true"><path d="M1.5 3 L8.5 3 L5 7.5 Z"></path></svg>'


def assets(t):
    return f'''<div style="flex: 1; overflow: auto; padding: 0 10px 12px">
<div style="display: flex; align-items: center; justify-content: center; gap: 10px; height: 64px; border-radius: 12px; border: 1.5px dashed {hexa(t['acc'], 0.6)}; font-size: 13px; font-weight: 500; color: #E6E6E8">Drop files anywhere</div>
<div style="margin-top: 12px">
<button class="srow" type="button">{TRI}Textures<span style="margin-left: auto; color: #8E8E95">0</span></button>
<button class="srow" type="button" style="color: #FFFFFF">{TRID}Materials<span style="margin-left: auto; color: #8E8E95">2</span></button>
<button class="srow" type="button" style="padding-left: 30px"><span style="width: 14px; height: 14px; border-radius: 4px; background: #CFC8BA"></span>Concrete</button>
<button class="srow" type="button" style="padding-left: 30px"><span style="width: 14px; height: 14px; border-radius: 4px; background: linear-gradient(135deg, #C9CDD3, #6A6E75)"></span>Brushed steel</button>
<button class="srow" type="button">{TRI}Models<span style="margin-left: auto; color: #8E8E95">1</span></button>
<button class="srow" type="button">{TRI}Sounds<span style="margin-left: auto; color: #8E8E95">3</span></button>
<button class="srow" type="button">{TRI}Other<span style="margin-left: auto; color: #8E8E95">0</span></button>
</div></div>'''


def dock(t, inspect_html, list_html, list_label='Scene', list_icon='layers'):
    segs = []
    for vid, label, icon in (('Inspect', 'Inspect', 'sliders'), ('Scene', list_label, list_icon), ('Assets', 'Assets', 'folder')):
        segs.append(f'<sc-if value="{{{{ is{vid} }}}}" hint-placeholder-val="{{{{ {str(vid == 'Inspect').lower()} }}}}"><button type="button" role="tab" aria-selected="true" style="flex: 1; display: flex; align-items: center; justify-content: center; gap: 6px; height: 28px; border-radius: 7px; background: {t['acc']}; color: {t['accFg']}; font-size: 12.5px; font-weight: 500">{ic(icon, 'currentColor', 13)}{label}</button></sc-if>'
                    f'<sc-if value="{{{{ not{vid} }}}}" hint-placeholder-val="{{{{ {str(vid != 'Inspect').lower()} }}}}"><button type="button" role="tab" onClick="{{{{ open{vid} }}}}" style="flex: 1; display: flex; align-items: center; justify-content: center; gap: 6px; height: 28px; border-radius: 7px; color: #B0B0B6; font-size: 12.5px">{ic(icon, 'currentColor', 13)}{label}</button></sc-if>')
    return f'''<sc-if value="{{{{ open }}}}" hint-placeholder-val="{{{{ true }}}}">
<aside aria-label="Dock" class="glass" style="position: absolute; right: 12px; top: 12px; bottom: 12px; width: 352px; display: flex; flex-direction: column; border-radius: 16px; overflow: hidden">
<div style="display: flex; align-items: center; gap: 6px; padding: 10px">
<div role="tablist" aria-label="Dock view" style="display: flex; flex: 1; padding: 3px; border-radius: 10px; background: rgba(255,255,255,0.05)">{"".join(segs)}</div>
<button type="button" class="ib" aria-label="Collapse dock" title="Collapse  ]" onClick="{{{{ toggle }}}}">{ic("right", "currentColor", 16)}</button>
</div>
<sc-if value="{{{{ isInspect }}}}" hint-placeholder-val="{{{{ true }}}}"><div style="flex: 1; overflow: auto">{inspect_html}</div></sc-if>
<sc-if value="{{{{ isScene }}}}" hint-placeholder-val="{{{{ false }}}}">{list_html}</sc-if>
<sc-if value="{{{{ isAssets }}}}" hint-placeholder-val="{{{{ false }}}}">{assets(t)}</sc-if>
</aside>
</sc-if>
<sc-if value="{{{{ closed }}}}" hint-placeholder-val="{{{{ false }}}}">{rail(t, True)}</sc-if>'''


def rail(t, live=False):
    on = lambda h: f' onClick="{{{{ {h} }}}}"' if live else ''
    b = 'style="width: 32px; height: 32px; border-radius: 8px; display: flex; align-items: center; justify-content: center; color: #D2D2D6"'
    return f'''<aside aria-label="Dock, collapsed" class="glass" style="position: absolute; right: 12px; top: 12px; width: 44px; display: flex; flex-direction: column; align-items: center; gap: 4px; padding: 6px 0; border-radius: 14px">
<button type="button" aria-label="Open dock" title="Open dock  ["{on('toggle')} {b}>{ic("left", "currentColor", 16)}</button>
<button type="button" aria-label="Inspect"{on('openInspect')} {b}>{ic("sliders", "currentColor", 16)}</button>
<button type="button" aria-label="Scene"{on('openScene')} {b}>{ic("layers", "currentColor", 16)}</button>
<button type="button" aria-label="Assets"{on('openAssets')} {b}>{ic("folder", "currentColor", 16)}</button>
</aside>'''


DOCK_SCRIPT = '''class Component extends DCLogic {
  constructor(props) { super(props); this.state = { view: 'inspect', open: true }; }
  renderVals() {
    const v = this.state.view;
    const go = (view) => () => this.setState({ open: true, view });
    return {
      open: this.state.open, closed: !this.state.open,
      isInspect: v === 'inspect', isScene: v === 'scene', isAssets: v === 'assets',
      notInspect: v !== 'inspect', notScene: v !== 'scene', notAssets: v !== 'assets',
      toggle: () => this.setState({ open: !this.state.open }),
      openInspect: go('inspect'), openScene: go('scene'), openAssets: go('assets')
    };
  }
}'''
STATIC_SCRIPT = '''class Component extends DCLogic {
  renderVals() { return {}; }
}'''


TOPIC_ICON = dict(All='grid', Physics='feel', Lighting='bulb', Cameras='camera', Water='drop', Foliage='leaf', World='globe',
                  Body='person', Moves='run', Camera='camera', Controls='keyboard', Drive='wheel', Seats='seat',
                  Layout='layout', Widgets='target', Fonts='text', Bindings='wire',
                  Plumes='flame', Particles='spark', Fire='heat', Post='aperture',
                  Sounds='wave', Music='music', Ambience='wind', Mixer='sliders', Voices='caption',
                  Shots='film', Captions='caption', Sound='music', Menus='monitor', Flow='route', Transitions='swap', Styles='palette')


def frame_alpha(t):
    return t.get('alpha', 0.86)


_TOPICS = {}
VX, VY, VW, VH, VR = 8, 44, 1584, 908, 14   # view rectangle in window coords
ND = 34                                       # notch depth


def notch(t, topics, active):
    _TOPICS['v'] = (topics, active)
    return ''


def topic_chips(t, topics, active):
    chips, w = [], 0
    for name in topics:
        soon = name.endswith('*')
        n = name.rstrip('*')
        w += len(n) * 7 + 44
        i = ic(TOPIC_ICON.get(n, 'grid'), 'currentColor', 14)
        if n == active:
            chips.append(f'<button class="chip" type="button" role="tab" aria-selected="true" style="background: {t["fg"]}; color: {t["bg"]}; font-weight: 500">{i}{n}</button>')
        elif soon:
            chips.append(f'<button class="chip" type="button" role="tab" title="Coming soon" style="opacity: 0.55">{i}{n}</button>')
        else:
            chips.append(f'<button class="chip" type="button" role="tab">{i}{n}</button>')
    return ''.join(chips), w + 2 * len(topics)


def outline(nt, nb, ox=0, oy=0):
    """View outline with a notch dipping in at top centre (width nt) and rising at bottom centre (width nb)."""
    x0, y0, x1, y1, r, d = VX - ox, VY - oy, VX + VW - ox, VY + VH - oy, VR, ND
    cx = x0 + VW / 2
    a1, a2 = cx - nt / 2, cx + nt / 2
    b1, b2 = cx - nb / 2, cx + nb / 2
    f = lambda v: f'{v:.1f}'.rstrip('0').rstrip('.')
    return (f'M{f(x0+r)} {f(y0)} H{f(a1)} C{f(a1+20)} {f(y0)} {f(a1+22)} {f(y0+d)} {f(a1+44)} {f(y0+d)} '
            f'H{f(a2-44)} C{f(a2-22)} {f(y0+d)} {f(a2-20)} {f(y0)} {f(a2)} {f(y0)} '
            f'H{f(x1-r)} A{r} {r} 0 0 1 {f(x1)} {f(y0+r)} V{f(y1-r)} A{r} {r} 0 0 1 {f(x1-r)} {f(y1)} '
            f'H{f(b2)} C{f(b2-20)} {f(y1)} {f(b2-22)} {f(y1-d)} {f(b2-44)} {f(y1-d)} '
            f'H{f(b1+44)} C{f(b1+22)} {f(y1-d)} {f(b1+20)} {f(y1)} {f(b1)} {f(y1)} '
            f'H{f(x0+r)} A{r} {r} 0 0 1 {f(x0)} {f(y1-r)} V{f(y0+r)} A{r} {r} 0 0 1 {f(x0+r)} {f(y0)} Z')


def frame_svg(t, inner):
    edge = 'rgba(255,255,255,0.2)' if t['fg'] != INK else 'rgba(14,14,16,0.22)'
    return (f'<svg width="1600" height="960" viewBox="0 0 1600 960" style="position: absolute; inset: 0; pointer-events: none" aria-hidden="true">'
            f'<defs><linearGradient id="sheen" x1="0" y1="0" x2="0" y2="64" gradientUnits="userSpaceOnUse"><stop offset="0" stop-color="#FFFFFF" stop-opacity="0.16"></stop><stop offset="1" stop-color="#FFFFFF" stop-opacity="0"></stop></linearGradient></defs>'
            f'<path fill-rule="evenodd" d="M0 0 H1600 V960 H0 Z {inner}" fill="{t["bg"]}" fill-opacity="{frame_alpha(t)}"></path>'
            f'<path fill-rule="evenodd" d="M0 0 H1600 V960 H0 Z {inner}" fill="url(#sheen)"></path>'
            f'<path d="{inner}" fill="none" stroke="{edge}" stroke-width="1"></path></svg>')


def page(tab, title, view_inner, script, hints=None, world='', view_bg='transparent'):
    t = THEMES[tab]
    mut = muted(t['fg'])
    tabs = []
    for name in TAB_ORDER:
        f = TAB_FILES.get(name)
        i = ic(TAB_ICON[name], 'currentColor', 15)
        if name == tab:
            tabs.append(f'<a class="tab" href="{f}" aria-current="page" style="background: {t["fg"]}; color: {t["bg"]}; font-weight: 500">{i}{name}</a>')
        elif f:
            tabs.append(f'<a class="tab" href="{f}">{i}{name}</a>')
        else:
            tabs.append(f'<button class="tab" type="button" title="Not built yet">{i}{name}</button>')
    hints = hints or [('Orbit', 'Middle-drag'), ('Pan', 'Space'), ('Zoom', 'Wheel'), ('Fly', 'Caps Lock')]
    hint_html = ''.join(f'<span style="display: inline-flex; align-items: center; gap: 6px">{a} <span class="kf">{k}</span></span>' for a, k in hints)
    nb = 220 + sum(len(a) * 6.6 + len(k) * 6.2 + 40 for a, k in hints)
    topics, active = _TOPICS.pop('v', ([], ''))
    chips, cw = topic_chips(t, topics, active)
    nt = cw + 96 if topics else 0
    inner = outline(nt, nb)
    clip = outline(nt, nb, VX, VY)
    tgl = f'<span class="tg" style="background: {t["fg"]}"><i style="left: 16px; background: {t["bg"]}"></i></span>'
    btn, btnFg = t.get('btn', t['fg']), t.get('btnFg', t['bg'])
    edge = 'rgba(255,255,255,0.14)' if t['fg'] != INK else 'rgba(14,14,16,0.35)'
    wc = ''.join(f'<button type="button" class="wc" aria-label="{lab}">{ic(n, "currentColor", 14)}</button>' for n, lab in (('minus', 'Minimise'), ('square', 'Maximise'), ('x', 'Close')))
    sep = f'<span style="width: 1px; height: 16px; background: {mut}; opacity: .35"></span>'
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
<link rel="preconnect" href="https://fonts.googleapis.com">
<link href="https://fonts.googleapis.com/css2?family=Roboto:wght@400;500;700&amp;family=Roboto+Condensed:wght@700&amp;family=Roboto+Mono:wght@400;500;700&amp;display=swap" rel="stylesheet">
<style>
{css(t)}</style>
</helmet>
<div style="position: relative; width: 1600px; height: 960px; overflow: hidden; border-radius: 10px; background: {INK}; font-family: Roboto, sans-serif; color: {TXT}">
{world}
<div style="position: absolute; left: {VX}px; top: {VY}px; width: {VW}px; height: {VH}px; clip-path: path('{clip}'); overflow: hidden; background: {view_bg}">
{view_inner}
</div>
{frame_svg(t, inner)}
<div style="position: absolute; left: 0; right: 0; top: 0; height: 44px; display: flex; align-items: center; justify-content: space-between; padding: 0 0 0 10px; box-sizing: border-box; color: {t['fg']}">
<button class="mb" type="button" aria-label="Main menu: File, Edit, View, Play, Help" style="height: 32px; gap: 8px; padding: 0 10px">
<span style="display: flex; align-items: flex-start; gap: 2px; font: 700 20px/1 'Roboto Condensed', sans-serif; letter-spacing: .02em">BEVERLY<svg width="9" height="9" viewBox="0 0 10 10" aria-hidden="true"><polygon points="5,0 6.2,3.6 10,3.6 6.9,5.8 8.1,9.5 5,7.2 1.9,9.5 3.1,5.8 0,3.6 3.8,3.6" fill="{t['acc'] if t['fg'] != INK else 'currentColor'}"></polygon></svg></span>{ic('down', 'currentColor', 14)}</button>
<nav aria-label="Workspaces" style="position: absolute; left: 50%; transform: translateX(-50%); display: flex; align-items: center; gap: 2px">{"".join(tabs)}</nav>
<div style="display: flex; align-items: center; gap: 4px; height: 44px">
<button type="button" style="display: inline-flex; align-items: center; gap: 7px; height: 30px; padding: 0 6px 0 10px; border-radius: 8px; background: {btn}; color: {btnFg}; font-size: 13px; font-weight: 500">{ic('cube', 'currentColor', 15)}Parts <span class="kf" style="background: {btnFg}; color: {btn}">Tab</span></button>
<button class="mb" type="button" aria-label="Undo" title="Undo  Ctrl Z" style="padding: 0 7px">{ic('undo', 'currentColor', 17)}</button>
<button class="mb" type="button" aria-label="Redo" title="Redo  Ctrl Y" style="padding: 0 7px">{ic('redo', 'currentColor', 17)}</button>
<button class="mb" type="button" aria-label="Snap on, 0.25 metres and 15 degrees" title="Snap  0.25 m · 15°  (S)" style="gap: 7px">{ic('grid', 'currentColor', 16)}{tgl}</button>
<span style="width: 1px; height: 20px; background: {t['fg']}; opacity: .2; margin: 0 4px"></span>
{wc}
</div>
</div>
<div role="tablist" aria-label="Topics" style="position: absolute; left: {VX + (VW - nt) / 2:.0f}px; width: {nt:.0f}px; top: {VY + 5}px; display: flex; justify-content: center; gap: 2px; color: {t['fg']}">{chips}</div>
<div style="position: absolute; left: {VX + (VW - nb) / 2:.0f}px; width: {nb:.0f}px; top: {VY + VH - ND + 4}px; height: 26px; display: flex; align-items: center; justify-content: center; gap: 14px; font-size: 12px; color: {t['fg']}">
<span style="display: inline-flex; align-items: center; gap: 6px; font-weight: 500">{ic('check', 'currentColor', 14, 2.5)}Saved</span>{sep}
<span style="display: flex; align-items: center; gap: 16px; color: {mut}">{hint_html}</span>{sep}
<button class="mb" type="button" style="height: 24px; gap: 6px; font-size: 12px; padding: 0 6px">{ic('clock', 'currentColor', 14)}Timeline <span class="kf">T</span></button>
</div>
<div style="position: absolute; inset: 0; border-radius: 10px; box-shadow: inset 0 0 0 1px {edge}; pointer-events: none"></div>
</div>
</x-dc>
<script type="text/x-dc" data-dc-script data-props='{{"$preview":{{"width":1600,"height":960}}}}'>
{script}
</script>
</body>
</html>
'''
