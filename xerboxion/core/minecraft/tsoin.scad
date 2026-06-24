// ============================================================================
// TSOIN — le premier item du mod minecraft-os (José). v3.
// Feedback José : « c'est un SABLIER, mais le trou du milieu est juste assez gros
//   comme un TUBE » + « un sablier BULKY ».
// => sablier épais/bulky : deux bols larges qui se rejoignent sur un TUBE central
//    (le col), traversé par un TROU (le passage du sable) "juste assez gros".
//    Paramétrique — on continue d'affiner.
// ============================================================================
$fn = 120;

H     = 26;     // hauteur totale
RBOWL = 13.5;   // rayon des bols (bulky = large)
RTUBE = 5.2;    // rayon du col (le "comme un tube")
HTUBE = 7;      // hauteur du tube central
HOLE  = 3.0;    // rayon du trou qui traverse (juste assez gros)
LIP   = 1.6;    // rebord des bols

module solide() {
    union() {
        // col central = un tube
        translate([0,0,(H-HTUBE)/2]) cylinder(h = HTUBE, r = RTUBE);
        // bol du haut : du tube (étroit) vers large
        translate([0,0,(H+HTUBE)/2]) cylinder(h = (H-HTUBE)/2, r1 = RTUBE, r2 = RBOWL);
        // bol du bas : miroir
        translate([0,0,(H-HTUBE)/2]) mirror([0,0,1]) cylinder(h = (H-HTUBE)/2, r1 = RTUBE, r2 = RBOWL);
        // rebords épais (bulky)
        translate([0,0,H-LIP]) cylinder(h = LIP, r1 = RBOWL*0.93, r2 = RBOWL);
        translate([0,0,0])     cylinder(h = LIP, r1 = RBOWL, r2 = RBOWL*0.93);
    }
}

module tsoin() {
    difference() {
        solide();
        // le trou-tube qui traverse tout le milieu
        translate([0,0,-1]) cylinder(h = H + 2, r = HOLE);
    }
}

tsoin();
