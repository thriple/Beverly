# Beverly icon set — 24 × 24 grid, 2 px stroke, round caps and joins, >= 2 px padding.
# Shared keylines: circle r 9 at (12,12), square 3.5..20.5, corner radius 1.5.
def _c(cx, cy, r):
    return f'M{cx+r} {cy} A{r} {r} 0 1 1 {cx-r} {cy} A{r} {r} 0 1 1 {cx+r} {cy} Z'


def _rr(x1, y1, x2, y2, r=1.5):
    return (f'M{x1+r} {y1} H{x2-r} A{r} {r} 0 0 1 {x2} {y1+r} V{y2-r} A{r} {r} 0 0 1 {x2-r} {y2} '
            f'H{x1+r} A{r} {r} 0 0 1 {x1} {y2-r} V{y1+r} A{r} {r} 0 0 1 {x1+r} {y1} Z')


P = dict(
    # part categories
    cube='M12 2.5 L20.5 7 V17 L12 21.5 L3.5 17 V7 Z M3.5 7 L12 11.5 L20.5 7 M12 11.5 V21.5',
    bolt='M13 2.5 L4.5 13.5 H12 L11 21.5 L19.5 10.5 H12 Z',
    play='M7 4.5 V19.5 L19 12 Z',
    route='M3.5 12 H9 C 12.5 12, 12.5 6, 16 6 H20.5 M9 12 C 12.5 12, 12.5 18, 16 18 H20.5 M18 3.5 L20.5 6 L18 8.5 M18 15.5 L20.5 18 L18 20.5',
    diamond='M12 3 L21 12 L12 21 L3 12 Z',
    person=_c(12, 7, 3.5) + ' M4.5 20.5 V19 A5 5 0 0 1 9.5 14 H14.5 A5 5 0 0 1 19.5 19 V20.5',
    bulb='M9 17.5 V16 C 9 15, 8.5 14.5, 7.8 13.8 A6.5 6.5 0 1 1 16.2 13.8 C 15.5 14.5, 15 15, 15 16 V17.5 Z M10 21 H14',
    camera=_rr(2.5, 6.5, 15.5, 17.5) + ' M15.5 10.5 L21.5 7.5 V16.5 L15.5 13.5',
    # tabs
    mountain='M2.5 19.5 L9 8 L13.5 15.5 L16 11.5 L21.5 19.5 Z',
    car='M3 16 V12.5 L5.4 7.6 A2 2 0 0 1 7.2 6.5 H16.8 A2 2 0 0 1 18.6 7.6 L21 12.5 V16 A1 1 0 0 1 20 17 H4 A1 1 0 0 1 3 16 Z M3 12.5 H21 M6.5 17 V19.5 M17.5 17 V19.5',
    layout=_rr(3.5, 3.5, 20.5, 20.5) + ' M3.5 9 H20.5 M9 9 V20.5',
    wave='M2.5 12 H5 L7.5 6 L10.5 18 L13.5 4 L16.5 20 L19 12 H21.5',
    flame='M12 2.5 C 16 6.5, 19 10, 19 14.5 A7 7 0 0 1 5 14.5 C 5 11, 7.5 9.5, 8.5 7 C 9.5 9.5, 10.5 10.5, 12 10.5 C 12.5 8, 12 5.5, 12 2.5 Z',
    monitor=_rr(3, 3.5, 21, 16.5) + ' M8 20.5 H16 M12 16.5 V20.5',
    film=_rr(3, 3.5, 21, 20.5, 2) + ' M7.5 3.5 V20.5 M16.5 3.5 V20.5 M3 12 H21',
    # topics and groups
    grid=_rr(3.5, 3.5, 20.5, 20.5) + ' M3.5 12 H20.5 M12 3.5 V20.5',
    feel=_c(12, 8, 4) + ' M3 20.5 H21 M12 15 V17.5',
    drop='M12 2.5 C 16 7.5, 18.5 10.5, 18.5 14.5 A6.5 6.5 0 0 1 5.5 14.5 C 5.5 10.5, 8 7.5, 12 2.5 Z',
    leaf='M5 19 C 5 10, 10 4.5, 20 4 C 19.5 14, 14 19, 5 19 Z M5 19 L13 11',
    globe=_c(12, 12, 9) + ' M3 12 H21 M12 3 C 9 6, 9 18, 12 21 C 15 18, 15 6, 12 3',
    run=_c(15, 4.5, 2) + ' M7 11 L11 8.5 L14.5 12 L18.5 12.5 M11 8.5 L9 15 L5 19 M9 15 L14 17 L14.5 21.5',
    keyboard=_rr(2.5, 6, 21.5, 18) + ' M6.5 10 H6.6 M10 10 H10.1 M13.9 10 H14 M17.4 10 H17.5 M8 14 H16',
    wheel=_c(12, 12, 8.5) + ' ' + _c(12, 12, 2.5) + ' M12 3.5 V9.5 M12 14.5 V20.5 M3.5 12 H9.5 M14.5 12 H20.5',
    seat='M7 3.5 V14 H17 A1.5 1.5 0 0 1 18.5 15.5 V20.5 M7 14 L5.5 20.5 M7 9 H13',
    target=_c(12, 12, 8.5) + ' ' + _c(12, 12, 2.5) + ' M12 2 V5.5 M12 18.5 V22 M2 12 H5.5 M18.5 12 H22',
    text='M5 5 H19 M12 5 V19.5 M9 19.5 H15',
    wire='M6 15.5 C 6 10, 18 14, 18 8.5 ' + _c(6, 18, 2.5) + ' ' + _c(18, 6, 2.5),
    spark='M12 2.5 V7 M12 17 V21.5 M2.5 12 H7 M17 12 H21.5 M5.3 5.3 L8.5 8.5 M15.5 15.5 L18.7 18.7 M18.7 5.3 L15.5 8.5 M8.5 15.5 L5.3 18.7',
    heat='M10 4.5 A2 2 0 0 1 14 4.5 V13.5 A4 4 0 1 1 10 13.5 Z M12 10 V17',
    aperture=_c(12, 12, 9) + ' M12 3 L14.6 12 M21 12 L12 14.6 M12 21 L9.4 12 M3 12 L12 9.4',
    caption=_rr(3, 4, 21, 17, 2) + ' M8 17 L5.5 20.5 M7.5 9 H16.5 M7.5 12.5 H13',
    music=_c(6, 18, 3) + ' M9 18 V5 L20.5 3 V16 ' + _c(17.5, 16, 3),
    swap=_rr(3, 5, 13, 15) + ' M11 9 H19.5 A1.5 1.5 0 0 1 21 10.5 V17.5 A1.5 1.5 0 0 1 19.5 19 H12.5 A1.5 1.5 0 0 1 11 17.5',
    palette='M12 3 A9 9 0 1 0 12 21 C 13.5 21, 14 20, 14 19 C 14 17.5, 13 17, 13 15.5 C 13 14, 14 13.5, 15.5 13.5 H17 A4 4 0 0 0 21 9.5 C 21 6, 17 3, 12 3 Z M7.5 11 H7.6 M10 7 H10.1 M15 7 H15.1',
    move='M12 2.5 V21.5 M2.5 12 H21.5 M9 5.5 L12 2.5 L15 5.5 M9 18.5 L12 21.5 L15 18.5 M5.5 9 L2.5 12 L5.5 15 M18.5 9 L21.5 12 L18.5 15',
    star='M12 2.5 L14.8 8.6 L21.5 9.3 L16.5 13.8 L17.9 20.5 L12 17.1 L6.1 20.5 L7.5 13.8 L2.5 9.3 L9.2 8.6 Z',
    pin='M12 21.5 C 7 16, 5 12.5, 5 9.5 A7 7 0 0 1 19 9.5 C 19 12.5, 17 16, 12 21.5 Z ' + _c(12, 9.5, 2.5),
    eye='M2.5 12 C 5 6.5, 19 6.5, 21.5 12 C 19 17.5, 5 17.5, 2.5 12 Z ' + _c(12, 12, 3),
    gauge='M3.5 17 A8.5 8.5 0 1 1 20.5 17 M12 15 L16.5 8.5 ' + _c(12, 15, 1.5),
    cone='M12 3 L20 20 H4 Z',
    wind='M3 8.5 H15 A3 3 0 1 0 12 5.5 M3 15.5 H18 A3 3 0 1 1 15 18.5 M3 12 H9',
    map='M3 6.5 L8.5 4 L15.5 6.5 L21 4 V17.5 L15.5 20 L8.5 17.5 L3 20 Z M8.5 4 V17.5 M15.5 6.5 V20',
    home='M3.5 11 L12 3.5 L20.5 11 M5.5 9.5 V20.5 H18.5 V9.5 M10 20.5 V14.5 H14 V20.5',
    pointer='M5 3.5 L19 11 L12.5 12.5 L10 19.5 Z',
    # actions
    sliders='M4 6 H20 M4 12 H20 M4 18 H20 M8 3.5 V8.5 M15 9.5 V14.5 M10 15.5 V20.5',
    layers='M12 2.5 L21 7.5 L12 12.5 L3 7.5 Z M3 12 L12 17 L21 12 M3 16.5 L12 21.5 L21 16.5',
    folder='M3 6.5 A1.5 1.5 0 0 1 4.5 5 H9 L11 7.5 H19.5 A1.5 1.5 0 0 1 21 9 V18 A1.5 1.5 0 0 1 19.5 19.5 H4.5 A1.5 1.5 0 0 1 3 18 Z',
    search=_c(10.5, 10.5, 7) + ' M15.5 15.5 L20.5 20.5',
    clock=_c(12, 12, 9) + ' M12 7 V12 L15.5 14',
    undo='M9 5 L4 10 L9 15 M4 10 H15 A5 5 0 0 1 15 20 H12',
    redo='M15 5 L20 10 L15 15 M20 10 H9 A5 5 0 0 0 9 20 H12',
    left='M15 5 L8 12 L15 19',
    right='M9 5 L16 12 L9 19',
    down='M6 9 L12 15 L18 9',
    copy=_rr(9, 9, 20.5, 20.5) + ' M15 9 V5 A1.5 1.5 0 0 0 13.5 3.5 H5 A1.5 1.5 0 0 0 3.5 5 V13.5 A1.5 1.5 0 0 0 5 15 H9',
    trash='M4 6.5 H20 M9.5 6.5 V4 H14.5 V6.5 M6 6.5 L7 20.5 H17 L18 6.5 M10 10.5 V16.5 M14 10.5 V16.5',
    reset='M4 12 A8 8 0 1 0 6.3 6.3 M4 4 V8.5 H8.5',
    plus='M12 5 V19 M5 12 H19',
    x='M6 6 L18 18 M18 6 L6 18',
    pipette='M14.5 5.5 L18.5 9.5 M17 4 A2.1 2.1 0 0 1 20 7 L16.5 10.5 L13.5 7.5 Z M15 9 L7 17 L4.5 19.5 L4.5 17 L12 9.5',
    align_l='M4 3.5 V20.5 M8 7 H17 M8 12 H20 M8 17 H14',
    align_ch='M12 3.5 V20.5 M7 7 H17 M5 12 H19 M8.5 17 H15.5',
    align_r='M20 3.5 V20.5 M7 7 H16 M4 12 H16 M10 17 H16',
    align_t='M3.5 4 H20.5 M7 8 V17 M12 8 V20 M17 8 V14',
    align_cv='M3.5 12 H20.5 M7 7 V17 M12 5 V19 M17 8.5 V15.5',
    align_b='M3.5 20 H20.5 M7 7 V16 M12 4 V16 M17 10 V16',
    dist_h='M4 3.5 V20.5 M20 3.5 V20.5 ' + _rr(9, 7, 15, 17, 1.5),
    dist_v='M3.5 4 H20.5 M3.5 20 H20.5 ' + _rr(7, 9, 17, 15, 1.5),
    lock=_rr(5, 11, 19, 20.5, 2) + ' M8 11 V8 A4 4 0 0 1 16 8 V11',
    unlock=_rr(5, 11, 19, 20.5, 2) + ' M8 11 V8 A4 4 0 0 1 15.6 6.3',
    scrub='M8 7 L3 12 L8 17 M16 7 L21 12 L16 17 M3 12 H21',
    info=_c(12, 12, 9) + ' M12 11 V16.5 M12 7.5 V7.6',
    dot=_c(12, 12, 4),
    minus='M5 12 H19',
    square=_rr(5.5, 5.5, 18.5, 18.5, 1.5),
    check='M4.5 12.5 L9.5 17.5 L19.5 6.5',
    alert='M12 3.5 L21.5 20 H2.5 Z M12 10 V14 M12 17 V17.1',
    loop='M17 3.5 L20.5 7 L17 10.5 M4 12 V10 A3 3 0 0 1 7 7 H20.5 M7 20.5 L3.5 17 L7 13.5 M20 12 V14 A3 3 0 0 1 17 17 H3.5',
    start='M6 5 V19 M19 5 L9 12 L19 19 Z',
    back='M10 6 L4 12 L10 18 M4 12 H20',
    # settings (a row's label icon)
    volume='M3.5 9.5 H7 L11.5 5.5 V18.5 L7 14.5 H3.5 Z M15 9 A4.2 4.2 0 0 1 15 15 M17.5 6 A8.5 8.5 0 0 1 17.5 18',
    pitch=_c(6.5, 17.5, 2.5) + ' M9 17.5 V4.5 L13 6.5 M18 4.5 V19.5 M15 7.5 L18 4.5 L21 7.5 M15 16.5 L18 19.5 L21 16.5',
    vary='M12 3.5 V13.5 M7 8.5 H17 M7 19.5 H17',
    near=_c(12, 12, 1.5) + ' ' + _c(12, 12, 5),
    range=_c(12, 12, 8.5) + ' ' + _c(12, 12, 1.5) + ' M14.5 12 H18.5 M16 9.5 L18.5 12 L16 14.5',
    shuffle='M3.5 7 H6.5 C 11.5 7, 12.5 17, 17.5 17 H20.5 M3.5 17 H6.5 C 11.5 17, 12.5 7, 17.5 7 H20.5 M18 4.5 L20.5 7 L18 9.5 M18 14.5 L20.5 17 L18 19.5',
)
# legacy names used by earlier boards
P['key'] = P['keyboard']
P['window'] = _rr(3, 4, 21, 20) + ' M3 8.5 H21'


def ic(name, color='currentColor', size=16, sw=None):
    if sw is None:
        sw = 2.25 if size <= 13 else 2
    return (f'<svg width="{size}" height="{size}" viewBox="0 0 24 24" fill="none" stroke="{color}" stroke-width="{sw}" '
            f'stroke-linecap="round" stroke-linejoin="round" aria-hidden="true" style="flex: none"><path d="{P[name]}"></path></svg>')


# what each icon means, for the reference board
GROUPS = [
    ('Part categories', ['cube', 'bolt', 'play', 'route', 'diamond', 'person', 'bulb', 'camera']),
    ('Tabs', ['mountain', 'person', 'car', 'layout', 'wave', 'flame', 'monitor', 'film']),
    ('Topics and groups', ['grid', 'feel', 'drop', 'leaf', 'globe', 'run', 'keyboard', 'wheel', 'seat', 'target', 'text', 'wire',
                           'spark', 'heat', 'aperture', 'caption', 'music', 'swap', 'palette', 'move', 'star', 'pin', 'eye', 'gauge',
                           'cone', 'wind', 'map', 'home', 'pointer']),
    ('Actions', ['sliders', 'layers', 'folder', 'search', 'clock', 'undo', 'redo', 'left', 'right', 'down', 'copy', 'trash',
                 'reset', 'plus', 'x', 'check', 'alert', 'loop', 'start', 'back', 'minus', 'square', 'pipette', 'lock', 'unlock', 'scrub', 'info']),
    ('Settings', ['volume', 'pitch', 'vary', 'near', 'range', 'shuffle']),
    ('Alignment', ['align_l', 'align_ch', 'align_r', 'align_t', 'align_cv', 'align_b', 'dist_h', 'dist_v']),
]
