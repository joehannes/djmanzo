#!/bin/bash
# The shop's pictures. Every shot is its own launch (shoot.py).
HERE=$(cd "$(dirname "$0")" && pwd)
D="python3 -I $HERE/shoot.py"
# 1. Organic Base, the default: every activity, and the special screens.
$D pkg-organic - organic-base dig=dig mix=mix perform=perform prepare=prepare requests=requests autopilot=autopilot practice=practice karaoke=karaoke event=event \
  launcher="mix;0" palette="mix;type:stems" guides="mix;click:1190:28" settings="mix;pal:Settings" effects="mix;click:30:368;click:822:368"
# 2. Every theme: the library open (Dig), and one more screen each.
EXTRA=(autopilot karaoke perform prepare event requests practice)
i=0
while IFS='|' read id slug; do
  extra=${EXTRA[$((i % 7))]}; i=$((i+1))
  $D "$id" - "$slug" dig=dig "$extra=$extra"
done <<'LIST'
pkg-daylight|daylight
pkg-studio|studio
pkg-booth|booth
pkg-industrial|industrial-techno
pkg-cyber|cyber-trance
pkg-sunset|sunset
pkg-watershed|watershed-living
pkg-club|club
pkg-festival|festival
pkg-caribbean|caribbean
pkg-latin|latin
pkg-wedding|wedding
pkg-lounge|lounge
pkg-scratch|scratch
pkg-stemlab|stem-lab
pkg-legible|colour-blind-safe
pkg-spectrum|spectrum
pkg-signal|signal
pkg-aurora|aurora
pkg-neon|karaoke-neon
pkg-spotlight|spotlight
pkg-velvet|velvet
pkg-malecon|malecon-night
pkg-carnaval|carnaval
pkg-biobay|bioluminescent-bay
pkg-rosa|bachata-rosa
LIST
# 3. The six skins, each on a theme it suits.
$D pkg-caribbean palms caribbean--palms dig=dig karaoke=karaoke
$D pkg-biobay waves bioluminescent-bay--waves dig=dig perform=perform
$D pkg-carnaval papel carnaval--papel-picado dig=dig karaoke=karaoke
$D pkg-neon halftone karaoke-neon--halftone karaoke=karaoke mix=mix
$D pkg-industrial trash-polka industrial-techno--trash-polka perform=perform dig=dig
$D pkg-velvet plankton velvet--glowing-plankton dig=dig autopilot=autopilot
# 4. Caribbean's special screens.
$D pkg-caribbean - caribbean launcher="mix;0" palette="mix;type:theme" effects="mix;click:30:368;click:822:368" guides="mix;click:1190:28"
echo BATCH3 DONE
