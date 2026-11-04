#!/usr/bin/env bash
# capture.sh [group...]: every take of the Tessera film, in a hidden, silent
# sandbox arena (IRICE_ARENA, default 7), with seeded demo data (account
# "Maya", the project's own wallpapers, canned AI answers from
# seed/mock-ai.py on 127.0.0.1:47311, which must be running). Nothing from a
# real screen. Groups: g m (close-ups at 2x, Glass / Material), full (1080p
# whole-screen takes). Raw takes land in raw/, then frames.sh makes clips/.
. "$(dirname "$0")/lib.sh"
QLMENU="$CAP/seed/menu.png"; QLTRAM="$CAP/seed/photos/tram-photo.jpg"

want() { [[ -z ${TAKES:-} || " $TAKES " == *" $1 "* ]]; } # (TAKES="cc share": only those)

closeups() { # <g|m>   (the pointer takes physical pixels: at 2x, twice the points)
    local L=$1
    want desktop && still "$L-desktop"
    # Island: earbuds connect, the rings fill
    want island && rec "$L-island" "560,0 800x200" 0.5 3.0 'ipc island demo parts'
    # Quick Settings: open, brightness up, caffeine on, then off
    if want cc; then
        rec "$L-cc" "1480,20 440x540" 0.4 1.0 '
ipc panels open controlCenter IRICE-TEST; sleep 1.3
P move 3180 720 glide 3180 470 700; sleep 0.6
P move 3516 508 click; sleep 1.1
P move 3516 508 click'
        inbox 'ipc panels close'; sleep 1
    fi
    # Sidebar: a question, the answer streams in; then the past chats
    if want ask; then
        rec "$L-ask" "0,0 470x1080" 0.4 1.0 '
ipc panels open sidebar IRICE-TEST; sleep 1.2
key "Plan a weekend in Lisbon"; sleep 0.4; key -k Return; sleep 6.5'
        still "$L-ask-end"
    fi
    if want chats; then
        inbox 'ipc panels open sidebar IRICE-TEST'; sleep 1.5
        rec "$L-chats" "0,0 470x1080" 0.4 1.2 'P move 800 208 click; sleep 1.6; P move 440 760 glide 440 1100 700'
        inbox 'key -k Escape; sleep 0.4; ipc panels close'; sleep 1
    fi
    # Live Text: Quick Preview, Text (it reads the picture first), drag across the menu, copy
    if want livetext; then
        inbox "ipc panels quicklook $QLMENU"; sleep 2.5
        rec "$L-livetext" "260,90 1400x950" 0.4 1.2 'key T; sleep 2.6; P move 1196 892 down glide 2660 1184 1300 up; sleep 0.9; key -M ctrl c -m ctrl'
        inbox 'ipc panels quicklook ""'; sleep 1
    fi
    # Quick Share: a phone asks, Accept, the count, done
    if want share; then
        rec "$L-share" "560,280 800x520" 0.5 1.5 'ipc sharingdemo ask; sleep 1.6; P move 2070 1276 click; sleep 0.5
for p in 8 19 33 47 61 74 86 95 100; do ipc sharingdemo progress $p; sleep 0.25; done; ipc sharingdemo done'
        inbox 'ipc panels close'; sleep 1
    fi
}

full() {
    # Login intro (Glass), the look switch (Glass to Material), circle to
    # search, dynamic colour: whole screen at 1080p
    setup glass "$WALLS/wall-sunset-lake.jpg" 1
    still "full-g-desktop"
    rec "full-g-intro" "0,0 1920x1080" 0.2 1.0 'ipc loginIntro play; sleep 3.5'
    inbox "ipc panels quicklook $QLTRAM"; sleep 2.5
    rec "full-g-circle" "0,0 1920x1080" 0.4 1.0 'ipc screen circle; sleep 1.0
P move 760 360 down glide 1100 330 220 glide 1240 470 220 glide 1150 680 220 glide 850 720 220 glide 700 560 220 glide 790 380 200 up; sleep 7.5'
    inbox 'ipc panels quicklook ""; ipc panels close'; sleep 1.5
    rec "full-g-colour" "0,0 1920x1080" 0.4 1.0 "
python3 $CAP/seed/seed.py $HOMEDIR glass $WALLS/wall-boats-lake.jpg >/dev/null; sleep 2.2
python3 $CAP/seed/seed.py $HOMEDIR glass $WALLS/wall-purple-path.jpg >/dev/null; sleep 2.2
python3 $CAP/seed/seed.py $HOMEDIR glass $WALLS/wall-golden-shore.jpg >/dev/null; sleep 2.2"
    python3 "$CAP/seed/seed.py" "$HOMEDIR" glass "$WALLS/wall-sunset-lake.jpg" >/dev/null; sleep 3
    rec "full-switch" "0,0 1920x1080" 0.4 2.0 "python3 $CAP/seed/seed.py $HOMEDIR material $WALLS/wall-sunset-lake.jpg >/dev/null; sleep 9"
    sleep 6
    still "full-m-desktop"
    inbox "ipc panels quicklook $QLTRAM"; sleep 2.5
    rec "full-m-circle" "0,0 1920x1080" 0.4 1.0 'ipc screen circle; sleep 1.0
P move 760 360 down glide 1100 330 220 glide 1240 470 220 glide 1150 680 220 glide 850 720 220 glide 700 560 220 glide 790 380 200 up; sleep 7.5'
    inbox 'ipc panels quicklook ""; ipc panels close'; sleep 1.5
    rec "full-m-colour" "0,0 1920x1080" 0.4 1.0 "
python3 $CAP/seed/seed.py $HOMEDIR material $WALLS/wall-boats-lake.jpg >/dev/null; sleep 2.6
python3 $CAP/seed/seed.py $HOMEDIR material $WALLS/wall-purple-path.jpg >/dev/null; sleep 2.6
python3 $CAP/seed/seed.py $HOMEDIR material $WALLS/wall-golden-shore.jpg >/dev/null; sleep 2.6"
}

# Takes that click or drag: at 1x (at 2x the sandbox's virtual pointer clicks
# miss the shell's panels). Then the real look switch (switch-look.sh, as
# Settings runs it) and Material's whole-screen takes.
ones() {
    setup glass "$WALLS/wall-sunset-lake.jpg" 1
    inbox 'ipc panels open sidebar IRICE-TEST'; sleep 1.8
    rec "g-chats1" "0,0 470x1080" 0.4 1.4 'P move 400 104 click; sleep 1.8; P move 220 380 glide 220 560 700'
    inbox 'key -k Escape; sleep 0.3; ipc panels close'; sleep 1
    inbox "ipc panels quicklook $QLMENU"; sleep 2.5
    rec "g-livetext1" "260,90 1400x950" 0.4 1.2 'key T; sleep 3.0; P move 598 446 down glide 1330 592 1300 up; sleep 1.0; key -M ctrl c -m ctrl'
    inbox 'ipc panels quicklook ""'; sleep 1.5
    rec "full-switch" "0,0 1920x1080" 0.4 2.0 "python3 $CAP/seed/seed.py $HOMEDIR material $WALLS/wall-sunset-lake.jpg >/dev/null
setsid -f $ROOT/shell/assets/scripts/switch-look.sh material >/dev/null 2>&1; sleep 11"
    sleep 4
    dev hyprctl eval 'hl.monitor({ output = "WAYLAND-1", disabled = true })' >/dev/null
    still "full-m-desktop"
    inbox 'ipc panels open sidebar IRICE-TEST'; sleep 1.8
    rec "m-chats1" "0,0 470x1080" 0.4 1.4 'P move 400 104 click; sleep 1.8; P move 220 380 glide 220 560 700'
    inbox 'key -k Escape; sleep 0.3; ipc panels close'; sleep 1
    inbox "ipc panels quicklook $QLMENU"; sleep 2.5
    rec "m-livetext1" "260,90 1400x950" 0.4 1.2 'key T; sleep 3.0; P move 598 446 down glide 1330 592 1300 up; sleep 1.0; key -M ctrl c -m ctrl'
    inbox 'ipc panels quicklook ""'; sleep 1.5
    inbox "ipc panels quicklook $QLTRAM"; sleep 2.5
    rec "full-m-circle" "0,0 1920x1080" 0.4 1.0 'ipc screen circle; sleep 1.0
P move 760 360 down glide 1100 330 220 glide 1240 470 220 glide 1150 680 220 glide 850 720 220 glide 700 560 220 glide 790 380 200 up; sleep 7.5'
    inbox 'ipc panels quicklook ""; ipc panels close'; sleep 1.5
    rec "full-m-colour" "0,0 1920x1080" 0.4 1.0 "
python3 $CAP/seed/seed.py $HOMEDIR material $WALLS/wall-boats-lake.jpg >/dev/null; sleep 2.6
python3 $CAP/seed/seed.py $HOMEDIR material $WALLS/wall-purple-path.jpg >/dev/null; sleep 2.6
python3 $CAP/seed/seed.py $HOMEDIR material $WALLS/wall-golden-shore.jpg >/dev/null; sleep 2.6"
}

# The film proper (built on the site's hero film): calm backgrounds, as the
# original was recorded. The island and share takes the original had with
# other companies' names, again with generic ones, at its framing (a
# 3840x2400 output at 2x, the leaves, the blue accent: they sit on its
# laptop's screen); the new chapters at 1x on the leaves; Material's colours.
calm() {
    local LEAVES="$ROOT/site/film/hero/assets/leaves.jpg" OCEAN="$ROOT/site/film/hero/assets/p-ocean.jpg" PETALS="$ROOT/site/film/hero/assets/p-petals.jpg"
    ACCENT=blue setup glass "$LEAVES" 2 "3840x2400@60"
    still "calm-g-desktop"
    rec "note" "640,0 640x360" 0.5 3.5 'ipc island demo notification'
    sleep 6
    rec "fastpair" "640,0 640x360" 0.5 2.0 'ipc island demo connect; sleep 1.4
P move 2600 520 glide 2192 96 4000; sleep 0.3; P click; sleep 3.2; ipc island demo parts; sleep 3'
    sleep 6
    rec "card" "640,400 640x400" 0.5 2.0 'ipc sharingdemo ask; sleep 1.1
P move 2500 1800 glide 2056 1384 4400; sleep 0.3; P click; sleep 0.4
for p in 6 14 25 37 50 63 76 88 97 100; do ipc sharingdemo progress $p; sleep 0.3; done; ipc sharingdemo done; sleep 1'
    inbox 'ipc panels close'; sleep 1
    ACCENT=blue setup glass "$LEAVES" 1
    still "calm-g-desktop1"
    inbox "ipc panels quicklook $QLTRAM"; sleep 2.5
    rec "circle" "0,0 1920x1080" 0.4 1.0 'ipc screen circle; sleep 1.0
P move 1113 567 down glide 1098 556 37 glide 1081 547 35 glide 1063 541 33 glide 1044 535 31 glide 1026 530 29 glide 1007 525 27 glide 987 521 25 glide 967 518 24 glide 946 518 22 glide 925 521 21 glide 905 526 20 glide 886 534 19 glide 867 542 18 glide 850 552 17 glide 833 563 17 glide 818 575 16 glide 804 588 16 glide 794 603 15 glide 786 619 15 glide 782 635 15 glide 779 651 14 glide 777 667 14 glide 775 683 14 glide 775 698 14 glide 775 714 14 glide 779 730 14 glide 785 745 14 glide 794 759 14 glide 805 772 14 glide 817 784 14 glide 830 796 14 glide 845 806 14 glide 861 816 14 glide 878 824 14 glide 897 829 14 glide 917 831 14 glide 937 831 14 glide 957 829 14 glide 977 826 14 glide 996 822 14 glide 1015 817 14 glide 1033 812 15 glide 1051 805 15 glide 1067 796 15 glide 1082 785 16 glide 1095 773 16 glide 1107 761 17 glide 1118 748 17 glide 1129 734 18 glide 1139 720 19 glide 1147 705 20 glide 1151 689 21 glide 1152 673 22 glide 1150 658 24 glide 1145 642 25 glide 1138 628 27 glide 1131 614 29 glide 1122 601 31 glide 1112 588 33 glide 1100 577 35 glide 1086 567 37 glide 1071 559 40 up; sleep 2.5'
    inbox 'ipc panels quicklook ""; ipc panels close'; sleep 1.5
    inbox "ipc panels quicklook $QLMENU"; sleep 2.5
    rec "livetext" "260,90 1400x950" 0.4 1.2 'key T; sleep 3.0; P move 598 446 down glide 1330 592 1300 up; sleep 1.0; key -M ctrl c -m ctrl'
    inbox 'ipc panels quicklook ""'; sleep 1.5
    rec "ask" "0,0 470x1080" 0.4 1.0 'ipc panels open sidebar IRICE-TEST; sleep 1.2
key "Plan a weekend in Lisbon"; sleep 0.4; key -k Return; sleep 6.5'
    rec "chats" "0,0 470x1080" 0.4 1.4 'P move 400 104 click; sleep 1.8; P move 220 380 glide 220 560 700'
    inbox 'key -k Escape; sleep 0.3; ipc panels close'; sleep 1
    setup material "$OCEAN" 1
    still "calm-m-desktop"
    rec "colour" "0,0 1920x1080" 0.4 1.0 "
python3 $CAP/seed/seed.py $HOMEDIR material $PETALS >/dev/null; sleep 2.6
python3 $CAP/seed/seed.py $HOMEDIR material $LEAVES >/dev/null; sleep 2.6
python3 $CAP/seed/seed.py $HOMEDIR material $OCEAN >/dev/null; sleep 2.6"
}

# Retakes of the two island takes, without the temperature alert
islands() {
    local LEAVES="$ROOT/site/film/hero/assets/leaves.jpg"
    ACCENT=blue setup glass "$LEAVES" 2 "3840x2400@60"
    rec "note" "640,0 640x360" 0.5 3.5 'ipc island demo notification'
    sleep 6
    rec "fastpair" "640,0 640x360" 0.5 2.0 'ipc island demo connect; sleep 1.4
P move 2600 520 glide 2192 96 4000; sleep 0.3; P click; sleep 3.2; ipc island demo parts; sleep 3'
}

# Retakes without the heat alert; the history click on the clock (with a
# chat open, the clock sits left of the new-chat pencil)
fix2() {
    local LEAVES="$ROOT/site/film/hero/assets/leaves.jpg" OCEAN="$ROOT/site/film/hero/assets/p-ocean.jpg" PETALS="$ROOT/site/film/hero/assets/p-petals.jpg"
    ACCENT=blue setup glass "$LEAVES" 1
    inbox "ipc panels quicklook $QLTRAM"; sleep 2.5
    rec "circle" "0,0 1920x1080" 0.4 1.0 'ipc screen circle; sleep 1.0
P move 1113 567 down glide 1098 556 37 glide 1081 547 35 glide 1063 541 33 glide 1044 535 31 glide 1026 530 29 glide 1007 525 27 glide 987 521 25 glide 967 518 24 glide 946 518 22 glide 925 521 21 glide 905 526 20 glide 886 534 19 glide 867 542 18 glide 850 552 17 glide 833 563 17 glide 818 575 16 glide 804 588 16 glide 794 603 15 glide 786 619 15 glide 782 635 15 glide 779 651 14 glide 777 667 14 glide 775 683 14 glide 775 698 14 glide 775 714 14 glide 779 730 14 glide 785 745 14 glide 794 759 14 glide 805 772 14 glide 817 784 14 glide 830 796 14 glide 845 806 14 glide 861 816 14 glide 878 824 14 glide 897 829 14 glide 917 831 14 glide 937 831 14 glide 957 829 14 glide 977 826 14 glide 996 822 14 glide 1015 817 14 glide 1033 812 15 glide 1051 805 15 glide 1067 796 15 glide 1082 785 16 glide 1095 773 16 glide 1107 761 17 glide 1118 748 17 glide 1129 734 18 glide 1139 720 19 glide 1147 705 20 glide 1151 689 21 glide 1152 673 22 glide 1150 658 24 glide 1145 642 25 glide 1138 628 27 glide 1131 614 29 glide 1122 601 31 glide 1112 588 33 glide 1100 577 35 glide 1086 567 37 glide 1071 559 40 up; sleep 2.5'
    inbox 'ipc panels quicklook ""; ipc panels close'; sleep 1.5
    rec "ask" "0,0 470x1080" 0.4 1.0 'ipc panels open sidebar IRICE-TEST; sleep 1.2
key "Plan a weekend in Lisbon"; sleep 0.4; key -k Return; sleep 6.5'
    still "fix2-ask-end"
    rec "chats" "0,0 470x1080" 0.4 1.4 'P move 362 104 click; sleep 1.8; P move 220 380 glide 220 560 700'
    inbox 'key -k Escape; sleep 0.3; ipc panels close'; sleep 1
    setup material "$OCEAN" 1
    rec "colour" "0,0 1920x1080" 0.4 1.0 "
python3 $CAP/seed/seed.py $HOMEDIR material $PETALS >/dev/null; sleep 2.6
python3 $CAP/seed/seed.py $HOMEDIR material $LEAVES >/dev/null; sleep 2.6
python3 $CAP/seed/seed.py $HOMEDIR material $OCEAN >/dev/null; sleep 2.6"
}

# The Material end on calm backgrounds: the switch (the Badge on its
# screen now), Quick Settings springing open, then the wallpaper changing
# with it open, so its tonal colours visibly follow
mat() {
    local LEAVES="$ROOT/site/film/hero/assets/leaves.jpg" OCEAN="$ROOT/site/film/hero/assets/p-ocean.jpg" PETALS="$ROOT/site/film/hero/assets/p-petals.jpg"
    ACCENT=blue setup glass "$OCEAN" 1
    inbox 'P move 1915 1075'  # (the pointer out of the way: the corner)
    rec "mswitch" "0,0 1920x1080" 0.3 1.5 "python3 $CAP/seed/seed.py $HOMEDIR material $OCEAN >/dev/null
setsid -f $ROOT/shell/assets/scripts/switch-look.sh material >/dev/null 2>&1; sleep 9"
    sleep 4
    dev hyprctl eval 'hl.monitor({ output = "WAYLAND-1", disabled = true })' >/dev/null
    inbox 'ipc island dismissNewest; ipc island clearHistory; ipc panels close'; sleep 2
    rec "mqs" "0,0 1920x1080" 0.4 1.2 'ipc panels open controlCenter IRICE-TEST; sleep 2'
    rec "colour" "0,0 1920x1080" 0.4 1.0 "
python3 $CAP/seed/seed.py $HOMEDIR material $PETALS >/dev/null; sleep 2.6
python3 $CAP/seed/seed.py $HOMEDIR material $LEAVES >/dev/null; sleep 2.6
python3 $CAP/seed/seed.py $HOMEDIR material $OCEAN >/dev/null; sleep 2.6"
    inbox 'ipc panels close'
}

# The circle drawn by hand round the tram (a smooth loop with a wobble, a
# little over one turn, tightening inside its start so it never crosses
# itself), and Live Text on a photographed bill (seed/photos/receipt-photo.jpg,
# 1200 px wide, the size tesseract reads every printed line at)
fix3() {
    local LEAVES="$ROOT/site/film/hero/assets/leaves.jpg"
    ACCENT=blue setup glass "$LEAVES" 1
    inbox "ipc panels quicklook $QLTRAM"; sleep 2.5
    rec "circle" "0,0 1920x1080" 0.4 1.0 'ipc screen circle; sleep 1.0
P move 1113 567 down glide 1098 556 37 glide 1081 547 35 glide 1063 541 33 glide 1044 535 31 glide 1026 530 29 glide 1007 525 27 glide 987 521 25 glide 967 518 24 glide 946 518 22 glide 925 521 21 glide 905 526 20 glide 886 534 19 glide 867 542 18 glide 850 552 17 glide 833 563 17 glide 818 575 16 glide 804 588 16 glide 794 603 15 glide 786 619 15 glide 782 635 15 glide 779 651 14 glide 777 667 14 glide 775 683 14 glide 775 698 14 glide 775 714 14 glide 779 730 14 glide 785 745 14 glide 794 759 14 glide 805 772 14 glide 817 784 14 glide 830 796 14 glide 845 806 14 glide 861 816 14 glide 878 824 14 glide 897 829 14 glide 917 831 14 glide 937 831 14 glide 957 829 14 glide 977 826 14 glide 996 822 14 glide 1015 817 14 glide 1033 812 15 glide 1051 805 15 glide 1067 796 15 glide 1082 785 16 glide 1095 773 16 glide 1107 761 17 glide 1118 748 17 glide 1129 734 18 glide 1139 720 19 glide 1147 705 20 glide 1151 689 21 glide 1152 673 22 glide 1150 658 24 glide 1145 642 25 glide 1138 628 27 glide 1131 614 29 glide 1122 601 31 glide 1112 588 33 glide 1100 577 35 glide 1086 567 37 glide 1071 559 40 up; sleep 2.5'
    inbox 'ipc panels quicklook ""; ipc panels close'; sleep 1.5
    inbox "P move 1915 1075; ipc panels quicklook $CAP/seed/photos/receipt-photo.jpg"; sleep 2.5
    rec "livetext" "260,90 1400x950" 0.4 1.5 'key T; sleep 3.2; P move 990 236 down glide 1384 508 1500 up; sleep 1.2; key -M ctrl c -m ctrl'
    inbox 'ipc panels quicklook ""'; sleep 1
}

# Circle to search on a water lily (seed/photos/lily-photo.jpg): the loop
# round the flower, above its reflection, then let go (the film shows the
# web result itself; the trail is drawn there from this same path, as the
# sandbox's software renderer stalls on it mid-stroke)
LILYPATH="move 1093 448 down glide 1080 440 37 glide 1065 433 35 glide 1049 427 33 glide 1033 422 31 glide 1016 418 29 glide 999 415 27 glide 982 412 25 glide 964 410 24 glide 945 410 22 glide 926 412 21 glide 908 415 20 glide 890 420 19 glide 873 427 18 glide 857 434 17 glide 842 441 17 glide 827 450 16 glide 815 460 16 glide 805 471 15 glide 797 482 15 glide 791 494 15 glide 787 506 14 glide 784 517 14 glide 782 529 14 glide 781 541 14 glide 782 553 14 glide 784 564 14 glide 789 576 14 glide 797 586 14 glide 806 596 14 glide 816 605 14 glide 828 614 14 glide 841 622 14 glide 855 629 14 glide 870 635 14 glide 887 639 14 glide 905 641 14 glide 923 641 14 glide 941 640 14 glide 959 638 14 glide 976 636 14 glide 993 633 14 glide 1010 629 15 glide 1026 623 15 glide 1042 617 15 glide 1055 609 16 glide 1068 601 16 glide 1080 592 17 glide 1091 582 17 glide 1101 572 18 glide 1110 562 19 glide 1117 550 20 glide 1121 539 21 glide 1123 527 22 glide 1122 515 24 glide 1119 504 25 glide 1114 493 27 glide 1108 482 29 glide 1101 472 31 glide 1092 463 33 glide 1082 454 35 glide 1070 447 37 glide 1056 440 40 up"
fix4() {
    local LEAVES="$ROOT/site/film/hero/assets/leaves.jpg"
    ACCENT=blue setup glass "$LEAVES" 1
    inbox "P move 1915 1075; ipc panels quicklook $CAP/seed/photos/lily-photo.jpg"; sleep 2.5
    rec "circle" "0,0 1920x1080" 0.4 1.0 "ipc screen circle; sleep 1.0
P $LILYPATH; sleep 1.5"
    inbox 'ipc panels quicklook ""; ipc panels close'; sleep 1
}

# The AI sidebar again, in a fresh session (seeded past chats only), so the
# chat and its history hold no Circle to search conversation
fix5() {
    local LEAVES="$ROOT/site/film/hero/assets/leaves.jpg"
    ACCENT=blue setup glass "$LEAVES" 1
    rec "ask" "0,0 470x1080" 0.4 1.0 'ipc panels open sidebar IRICE-TEST; sleep 1.2
key "Plan a weekend in Lisbon"; sleep 0.4; key -k Return; sleep 6.5'
    rec "chats" "0,0 470x1080" 0.4 1.4 'P move 362 104 click; sleep 1.8; P move 220 380 glide 220 560 700'
    inbox 'key -k Escape; sleep 0.3; ipc panels close'
}

# Round 3 (the mock AI must be running):
#   spotask    Search answers a question itself, then "Continue in chat"
#              carries it into the sidebar (Enter, Enter)
#   lockseq    Bedside Mode's widgets, someone back (the morph to the lock screen),
#              the reader and the camera asking, a match: unlocked
#              (`lock demo`: the sandbox has no charger, reader or camera)
#   glassramp  (now fix7, over tiles) the Glass page's Advanced sliders beside Quick Settings while
#              seed/glass-ramp.py takes the glass from flat to its strongest
fix6() {
    local LEAVES="$ROOT/site/film/hero/assets/leaves.jpg"
    ACCENT=blue setup glass "$LEAVES" 1
    inbox "P move 1915 1075"
    rec "spotask" "0,0 1920x1080" 0.4 1.2 'ipc panels open spotlight IRICE-TEST; sleep 0.9
key "Plan a weekend in Lisbon"; sleep 0.7; key -k Return; sleep 4.2; key -k Return; sleep 3.5'
    inbox 'ipc panels close'; sleep 1
    inbox 'ipc lock lock'; sleep 2.5
    inbox 'ipc lock demo standby'; sleep 2.5
    inbox 'key -k Right'; sleep 2
    rec "lockseq" "0,0 1920x1080" 0.3 0.8 'sleep 1.6; ipc lock demo wake; sleep 1.6; ipc lock demo finger; sleep 1.4; ipc lock demo face; sleep 1.4; ipc lock demo match; sleep 2.2'
    inbox 'ipc lock demo off'; sleep 1
}

# The glass ramp again, over tiles (their edges show the bending; the leaves
# hid it): IOR 1.33 up to 2, over 25% frost and full tint
fix7() {
    ACCENT=blue setup glass "$CAP/seed/photos/wall-tiles.jpg" 1
    python3 "$CAP/seed/glass-ramp.py" "$HOMEDIR" flat; sleep 1.5
    inbox 'ipc panels settings appearance-glass'; sleep 2.5
    inbox 'P move 1302 348 click; sleep 1; P move 1915 1075; sleep 0.3; ipc panels open controlCenter IRICE-TEST'; sleep 2
    still "glassramp-start"
    rec "glassramp" "0,0 1920x1080" 0.6 1.2 "sleep 0.8; python3 $CAP/seed/glass-ramp.py $HOMEDIR 2.6 40; sleep 1.2"
    inbox 'key -k Escape; ipc panels close'
}

# Bedside Mode again, in the order the film tells it: the widgets, a slide to the
# clock, someone back (the morph to the lock screen), the camera looking,
# the reader asking, the match (its check), then unlocked
fix8() {
    local LEAVES="$ROOT/site/film/hero/assets/leaves.jpg"
    ACCENT=blue setup glass "$LEAVES" 1
    inbox "P move 1915 1075; ipc lock lock"; sleep 2.5
    inbox 'ipc lock demo standby'; sleep 2.5
    inbox 'key -k Right'; sleep 2.5
    rec "lockseq" "0,0 1920x1080" 0.3 2.0 'sleep 1.4; key -k Left; sleep 1.6; ipc lock demo wake; sleep 1.5; ipc lock demo face; sleep 1.2; ipc lock demo finger; sleep 1.3; ipc lock demo match; sleep 0.9; ipc lock unlock; sleep 3'
    inbox 'ipc lock demo off'
}

# The island's volume row again (the iRice film's take named a branded pair
# of earbuds): the shell's "output" demo, the level stepping up in the
# arena's own (silent) PipeWire, at the island takes' framing
fix9() {
    local LEAVES="$ROOT/site/film/hero/assets/leaves.jpg"
    ACCENT=blue setup glass "$LEAVES" 2 "3840x2400@60"
    rec "vol" "640,0 640x360" 0.1 1.2 'ipc island demo output; sleep 0.5; for i in 1 2 3 4; do wpctl set-volume @DEFAULT_AUDIO_SINK@ 6%+; sleep 0.18; done; sleep 1'
}

for g in "${@:-g m full}"; do
    case $g in
        g) setup glass "$WALLS/wall-sunset-lake.jpg" 2; closeups g ;;
        m) setup material "$WALLS/wall-alpine-lake.jpg" 2; closeups m ;;
        full) full ;;
        ones) ones ;;
        calm) calm ;;
        islands) islands ;;
        fix2) fix2 ;;
        mat) mat ;;
        fix3) fix3 ;;
        fix4) fix4 ;;
        fix5) fix5 ;;
        fix6) fix6 ;;
        fix7) fix7 ;;
        fix8) fix8 ;;
        fix9) fix9 ;;
    esac
done
