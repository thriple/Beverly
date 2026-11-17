import os
import sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pal import oklch
from base import CAT, AX, INK

GRID = open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'grid.txt')).read()
sky_top = oklch(0.64, 0.12, 245); sky_mid = oklch(0.80, 0.07, 238); sky_hz = oklch(0.93, 0.03, 225)
hill_far = oklch(0.78, 0.04, 215); hill_near = oklch(0.66, 0.06, 170)
grass_hz = oklch(0.74, 0.09, 128); grass_near = oklch(0.55, 0.11, 135)
conc_lit = oklch(0.86, 0.025, 80); conc_mid = oklch(0.78, 0.028, 78); conc_sh = oklch(0.64, 0.03, 75)
floor = oklch(0.82, 0.025, 82); wood = oklch(0.55, 0.07, 60); sun = oklch(0.97, 0.06, 95)


def cloud(x, y, s):
    return (f'<g transform="translate({x} {y}) scale({s})" fill="#FFFFFF" opacity=".9"><ellipse cx="0" cy="0" rx="60" ry="16"></ellipse>'
            f'<ellipse cx="-22" cy="-12" rx="28" ry="18"></ellipse><ellipse cx="18" cy="-16" rx="34" ry="22"></ellipse></g>')


def backdrop(p):
    return f'''<defs>
<linearGradient id="{p}sky" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="{sky_top}"></stop><stop offset="0.6" stop-color="{sky_mid}"></stop><stop offset="1" stop-color="{sky_hz}"></stop></linearGradient>
<linearGradient id="{p}grass" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="{grass_hz}"></stop><stop offset="1" stop-color="{grass_near}"></stop></linearGradient>
<radialGradient id="{p}sun" cx="0.5" cy="0.5" r="0.5"><stop offset="0" stop-color="{sun}" stop-opacity=".9"></stop><stop offset="0.25" stop-color="{sun}" stop-opacity=".45"></stop><stop offset="1" stop-color="{sun}" stop-opacity="0"></stop></radialGradient>
</defs>
<rect x="-40" y="-80" width="1680" height="324" fill="url(#{p}sky)"></rect>
<circle cx="250" cy="120" r="150" fill="url(#{p}sun)"></circle>
<circle cx="250" cy="120" r="22" fill="#FFFFFF"></circle>
{cloud(560, 110, 1)}{cloud(1180, 70, 1.3)}{cloud(860, 160, .7)}
<path d="M-40 214 L0 214 C 160 170, 300 200, 440 186 S 720 150, 900 190 S 1250 168, 1588 196 L1640 196 V 244 H -40 Z" fill="{hill_far}"></path>
<path d="M-40 236 L0 236 C 220 212, 380 230, 560 222 S 980 206, 1200 226 S 1460 214, 1588 228 L1640 228 V 244 H -40 Z" fill="{hill_near}"></path>
<rect x="-40" y="242" width="1680" height="700" fill="url(#{p}grass)"></rect>
<g stroke="#FFFFFF" stroke-opacity="0.16" stroke-width="1">{GRID}</g>
<rect x="-40" y="236" width="1680" height="20" fill="{sky_hz}" opacity=".35"></rect>'''


def room(sel, acc):
    out = lambda on: f' stroke="{acc}" stroke-width="3"' if on else ''
    s = f'''<polygon points="974,390 1121,586 1300,600 1110,392" fill="#1C2A16" opacity=".22"></polygon>
<polygon points="569,390 974,390 1121,586 385,586" fill="{floor}"></polygon>
<polygon points="569,390 974,390 974,255 569,255" fill="{conc_lit}"></polygon>
<polygon points="974,390 1121,586 1121,341 974,255" fill="{conc_sh}"></polygon>
<polygon points="1040,478 1077,527 1077,368 1040,339" fill="#3A3631"></polygon>
<polygon points="1040,478 1004,497 1004,362 1040,339" fill="{wood}"{out(sel == 'door')}></polygon>
<polygon points="569,390 385,586 385,341 569,255" fill="{conc_mid}"{out(sel == 'wall')}></polygon>
<radialGradient id="lglow" cx="0.5" cy="0.5" r="0.5"><stop offset="0" stop-color="{CAT['light']}" stop-opacity="0.5"></stop><stop offset="1" stop-color="{CAT['light']}" stop-opacity="0"></stop></radialGradient>
<ellipse cx="740" cy="300" rx="70" ry="70" fill="url(#lglow)"></ellipse>
<circle cx="740" cy="300" r="9" fill="#FFFFFF" stroke="{CAT['light']}" stroke-width="3"></circle>
<polygon points="985,540 1085,540 1108,585 962,585" fill="{CAT['trig']}" fill-opacity="0.25" stroke="{CAT['trig']}" stroke-width="2" stroke-dasharray="6 4"></polygon>
<path d="M1035 540 C 1030 500, 1010 470, 1022 430" fill="none" stroke="{CAT['act']}" stroke-width="2.5" stroke-dasharray="4 4"></path>
<circle cx="1035" cy="540" r="5" fill="{CAT['trig']}" stroke="#FFFFFF" stroke-width="1.5"></circle>
<circle cx="1022" cy="430" r="5" fill="{CAT['act']}" stroke="#FFFFFF" stroke-width="1.5"></circle>
<ellipse cx="770" cy="500" rx="22" ry="8" fill="none" stroke="{CAT['game']}" stroke-width="2.5"></ellipse>
<rect x="762" y="456" width="16" height="40" rx="8" fill="{CAT['game']}"></rect>
<g transform="translate(860 214)" stroke="{CAT['cam']}" stroke-width="2.5" fill="{INK}" fill-opacity=".35" stroke-linejoin="round"><rect x="0" y="0" width="22" height="15" rx="3"></rect><path d="M22 4 l9 -4 v15 l-9 -4 Z"></path></g>'''
    if sel == 'wall':
        s += gizmo(477, 393)
    return s


def gizmo(x, y):
    return f'''<g stroke-linecap="round" stroke-width="3.5">
<line x1="{x}" y1="{y}" x2="{x}" y2="{y-80}" stroke="{AX[1]}"></line>
<line x1="{x}" y1="{y}" x2="{x+76}" y2="{y-37}" stroke="{AX[0]}"></line>
<line x1="{x}" y1="{y}" x2="{x-52}" y2="{y+57}" stroke="{AX[2]}"></line></g>
<polygon points="{x},{y-90} {x-6},{y-76} {x+6},{y-76}" fill="{AX[1]}"></polygon>
<polygon points="{x+85},{y-41} {x+71},{y-43} {x+77},{y-31}" fill="{AX[0]}"></polygon>
<polygon points="{x-58},{y+64} {x-55},{y+49} {x-45},{y+58}" fill="{AX[2]}"></polygon>
<circle cx="{x}" cy="{y}" r="5.5" fill="#FFFFFF" stroke="{INK}" stroke-width="1.5"></circle>'''


def svg(inner, p='s', w='1600', h='960', vb='-8 -44 1600 960', style='position: absolute; inset: 0; display: block'):
    return (f'<svg width="{w}" height="{h}" viewBox="{vb}" preserveAspectRatio="xMidYMid slice" style="{style}" aria-hidden="true">'
            f'{backdrop(p)}{inner}</svg>')
