import math
def lin(c): return c/12.92 if c<=0.04045 else ((c+0.055)/1.055)**2.4
def gam(c): return 12.92*c if c<=0.0031308 else 1.055*c**(1/2.4)-0.055
def hex2oklch(h):
    r,g,b=[lin(int(h[i:i+2],16)/255) for i in (1,3,5)]
    l=0.4122214708*r+0.5363325363*g+0.0514459929*b; m=0.2119034982*r+0.6806995451*g+0.1073969566*b; s=0.0883024619*r+0.2817188376*g+0.6299787005*b
    l,m,s=[x**(1/3) for x in (l,m,s)]
    L=0.2104542553*l+0.7936177850*m-0.0040720468*s; a=1.9779984951*l-2.4285922050*m+0.4505937099*s; bb=0.0259040371*l+0.7827717662*m-0.8086757660*s
    return L,math.hypot(a,bb),math.degrees(math.atan2(bb,a))%360
def oklch(L,C,H):
    a=C*math.cos(math.radians(H)); b=C*math.sin(math.radians(H))
    l=(L+0.3963377774*a+0.2158037573*b)**3; m=(L-0.1055613458*a-0.0638541728*b)**3; s=(L-0.0894841775*a-1.2914855480*b)**3
    r=4.0767416621*l-3.3077115913*m+0.2309699292*s; g=-1.2684380046*l+2.6097574011*m-0.3413193965*s; bl=-0.0041960863*l-0.7034186147*m+1.7076147010*s
    return '#'+''.join(f'{round(max(0,min(1,gam(max(0,x))))*255):02X}' for x in (r,g,bl))
def rgb(h): return ','.join(str(int(h[i:i+2],16)) for i in (1,3,5))
UI_H=262
neutrals="#0A0809 #F2F0F3 #1F1B1D #3A3438 #D9D4DA #CFC9D0 #1C181A #9C969E #ADA7AF #1A1618 #2A2528 #4A4448 #8E8890 #171315 #2E292C #151213 #050405 #0C0A0B #8B858D #B8B2BA #DAD5DB".split()
M={}
for h in neutrals:
    L,C,H=hex2oklch(h); M[h]=oklch(L, 0.006 if L>0.85 else 0.016, UI_H)
CL,CC=0.74,0.13
cat={'#8A93A6':oklch(0.70,0.035,UI_H),'#F08C42':oklch(CL,CC,55),'#F2C14E':oklch(0.82,0.135,95),'#48C08C':oklch(CL,CC,150),
 '#5EC8D8':oklch(CL,CC,205),'#5B9BF0':oklch(CL,CC,255),'#A77CE6':oklch(CL,CC,300),'#E0688F':oklch(CL,CC,350),
 '#F06A60':oklch(0.70,0.16,28),'#5CC25C':oklch(0.70,0.16,142),'#6A93F5':oklch(0.70,0.16,262),
 '#F5DC4A':oklch(0.87,0.15,88),'#FFF0A0':oklch(0.94,0.09,88),'#FF5E3A':oklch(0.68,0.19,35),'#FF8A6E':oklch(0.78,0.12,35),
 '#C0EF8F':oklch(0.87,0.13,135),'#BFD6F8':oklch(0.88,0.05,255)}
M.update(cat)
if __name__=='__main__':
    for k,v in M.items(): print(k,'->',v)
