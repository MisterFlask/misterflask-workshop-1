"""Install the atlas units: drop the legacy definitions they replace, add units/atlas.spr, fix the
manifest (source line, rules that would hijack the new art, remaps rewritten for the new keys)."""
import json, os, re, sys

R, A = sys.argv[1], sys.argv[2]  # revamp dir, web/art dir
mapping = json.load(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'unit_mapping.json')))
names = {k for k in mapping if not k.startswith('_')}
U = f'{A}/units'

removed = 0
for fn in sorted(os.listdir(U)):
    if not fn.endswith('.spr') or fn == 'atlas.spr':
        continue
    lines = open(f'{U}/{fn}').read().split('\n')
    out, i = [], 0
    while i < len(lines):
        m = re.match(r'^=\s*(\S+)\s', lines[i])
        if m and m.group(1) in names:
            i += 1
            while i < len(lines) and lines[i].strip() and not lines[i].lstrip().startswith('='):
                i += 1
            # swallow one separating blank line
            if i < len(lines) and not lines[i].strip():
                i += 1
            removed += 1
            continue
        out.append(lines[i])
        i += 1
    open(f'{U}/{fn}', 'w').write('\n'.join(out))
print('legacy definitions removed:', removed)

spr = open(f'{R}/out/units_atlas.spr').read()
open(f'{U}/atlas.spr', 'w').write(spr)

man = open(f'{U}/manifest.txt').read()
if 'source atlas.spr' not in man:
    man = man.replace('source generic.spr\n', 'source atlas.spr\nsource generic.spr\n', 1)
fixes = {
    # rules that would override a bespoke atlas sprite
    'map unit UNIT_FIREBOW -> unit/longbowman\n': '',
    'map unit UNIT_CENTAUR_CHARGER|UNIT_CENTAUR_LANCER -> unit/centaur': 'map unit UNIT_CENTAUR_CHARGER -> unit/centaur',
    'map unit UNIT_BISON_RIDER|UNIT_HORNGUARD -> unit/boar_rider': 'map unit UNIT_HORNGUARD -> unit/boar_rider',
    'map unit UNIT_IRA|UNIT_STYGIAN_GUARD -> unit/chaos_marauder': 'map unit UNIT_IRA -> unit/chaos_marauder',
    'map unit UNIT_SHADOWRIDER -> unit/knight remap=B2,D3,H5,G4,53,64\n': '',
    'map unit UNIT_OPHANIM -> unit/knight remap=B5,D6,GH,35,46,H8\n': '',
    # variant remaps, rewritten for the atlas sprites' keys (skin a b, leather i j h r, greys B D G H)
    'unit/lion remap=yY,uy': 'unit/lion remap=ij,ui,yY,hu',
    'unit/bear remap=uF,Q8,2g': 'unit/bear remap=uG,hD,iH,j8,rD',
    'unit/warrior remap=QQ,uc': 'unit/warrior remap=hc,uc,rI,jr,iI',
    'unit/scout remap=Qo,uq': 'unit/scout remap=ao,bm,Bw,Dz,GA',
    'unit/beastmaster remap=Qo,uq': 'unit/beastmaster remap=ao,bm,Bw,Dz,GA',
    'unit/archer remap=Qo': 'unit/archer remap=uz,jA,iz',
    'unit/archer remap=4f,3g,Qf': 'unit/archer remap=aF,bf,ug,ig,jf,rN',
    'unit/scout remap=Qf,ug': 'unit/scout remap=aF,bf,ug,hN,Bg,Df,GF',
    'unit/wolf_rider remap=of,Of,mg': 'unit/wolf_rider remap=aF,bf,Dg,Bg,3N,4g',
    'unit/knight remap=36,47,25': 'unit/knight remap=B5,D6,GH,35,46,H8',
    'unit/spectre remap=42,53 ghost': 'unit/spectre remap=53,64,f5,HD,42 ghost',
    'unit/spectre remap=4p,5P ghost': 'unit/spectre remap=4p,5P,6P,fP ghost',
    'unit/knight remap=43,54,65': 'unit/knight remap=B2,D3,H5,G4,53,64',
    'unit/knight remap=*C': 'unit/knight remap=BI,DC,GR',
    'unit/chaos_marauder remap=CT,cT': 'unit/chaos_marauder remap=It,ck,CT,RS',
}
for a, b in fixes.items():
    if a not in man:
        print('WARN not found:', a.strip())
    man = man.replace(a, b)
open(f'{U}/manifest.txt', 'w').write(man)
print('manifest updated')
