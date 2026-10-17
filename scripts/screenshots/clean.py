"""clean.py IN OUT -- remove the 'UI n fps' no-GPU warning pill from the top bar.
Finds the pill as the warn-coloured outline in the bar's middle row band and fills
its box, row by row, with the bar colour just left of it."""
import sys, numpy as np
from PIL import Image
im=np.array(Image.open(sys.argv[1]).convert("RGB")).astype(int)
y0,y1=14,44
band=im[y0:y1, 500:1100]
r,g,b=band[...,0],band[...,1],band[...,2]
warm=(r>170)&(g>110)&(b<120)&(r-b>90)
cols=np.where(warm.sum(0)>=2)[0]
if len(cols):
    # the pill: the first run of warm columns with gaps under 12 px
    runs=[[cols[0],cols[0]]]
    for c in cols[1:]:
        if c-runs[-1][1]<=12: runs[-1][1]=c
        else: runs.append([c,c])
    a,z=max(runs,key=lambda q:q[1]-q[0])
    if 30<=z-a<=120:
        x0,x1=500+a-3,500+z+4
        rows=np.where(warm[:,a:z+1].any(1))[0]
        ya,yb=y0+rows.min()-3,y0+rows.max()+4
        for y in range(ya,yb):
            im[y,x0:x1]=im[y,x0-4]
        print("cleaned",x0,x1,ya,yb)
Image.fromarray(im.astype(np.uint8)).save(sys.argv[2])
