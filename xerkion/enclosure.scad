// ============================================================================
//  XERKION #1 — boitier cube parametrique (le premier Kion physique)
//  Materialise le cub4ion. Unites = mm. Impression PETG. Inserts laiton M2.5.
//  TABLE DE VERITE (sérigraphiée sur le HAT, recopiée ici) :
//    Ecran ST7789 SPI0 : MOSI=GPIO10 SCLK=GPIO11 CS=GPIO8 DC=GPIO25 RST=GPIO27
//                        BL=GPIO13 (PWM1)  [JAMAIS 12/18]
//    IMU LSM6DS3 I2C1  : SDA=GPIO2 SCL=GPIO3  @0x6A   (axes // faces du cube)
//    Audio MAX98357A   : BCLK=GPIO18 LRCLK=GPIO19 DIN=GPIO21  SD=tie VDD 100k
//    Levier            : GPIO17 (Hall, pull-up interne)
//  VERIFIE : compile sous OpenSCAD 2021.01, solide manifold (Simple: yes).
//  Export : decommenter la piece voulue tout en bas, puis F6 -> Export STL.
// ============================================================================

// -------- PARAMETRE MAITRE : "il en faut de toutes les tailles" -------------
//   change scr_win (fenetre visible) -> tout se recalcule.
//   1.54" 240x240 carre : scr_win=28 (actif 27.7)  -> cube ~70 mm
//   2.1"  480x480       : scr_win=41                -> cube ~78 mm
//   4"    720x720       : scr_win=72                -> cube domine par l'ecran
scr_win     = 28.0;   // cote de la FENETRE VISIBLE de l'ecran carre (mm)
scr_pcb     = 38.0;   // cote du module PCB ecran (>= fenetre) -> rebord/lamage
scr_z       = 4.0;    // epaisseur module ecran (verre+PCB+FPC plie)

// -------- parois / tolerances / pieces --------------------------------------
wall        = 2.4;    // paroi cube (3 perimetres PETG @0.4 + marge)
clr         = 0.3;    // jeu d'impression PETG (logements)
fillet      = 1.6;    // arrondi aretes externes

// Raspberry Pi Zero 2 W : 65 x 30 x 5 ; trous M2.5 entraxe 58 x 23
pi_l = 65.0; pi_w = 30.0;  pi_hole_dx = 58.0; pi_hole_dy = 23.0;
// Batterie PiSugar/LiPo a plat (compartiment isole) ~ 52 x 36 x 8
batt_l = 52.0; batt_w = 36.0;
// IMU LSM6DS3 / GY-521 ~ 21.5 x 16 ; berceau coplanaire au fond (axes alignes)
acc_w = 22.0; acc_l = 18.0; acc_seat_h = 3.0; acc_hole_dx = 15.0;
// Haut-parleur rond
hp_d = 28.0; hp_grid_d = 24.0;
// fixations
ins_d = 3.5; ins_h = 6.0; post = 6.0;  // inserts laiton M2.5

$fn = 64;

// -------- DERIVE : cote du cube (max des 3 contraintes reelles) -------------
bezel_int    = 4.0;
besoin_ecran = scr_pcb + 2*bezel_int + 2*wall;          // impose par l'ecran
besoin_piece = max(pi_l, batt_l) + 2*(wall + 2);        // impose par la grande piece
inner        = max(besoin_ecran, besoin_piece, 65.2);   // cote interieur
outer        = inner + 2*wall;                           // cote exterieur

echo("=== XERKION ===");
echo(cote_externe_mm = outer);
echo(cote_interne_mm = inner);
echo(decoupe_ecran_mm = scr_win + 2*clr);
echo(entraxe_vis_Pi_mm = [pi_hole_dx, pi_hole_dy]);

// -------- utilitaires -------------------------------------------------------
module rcube(s, r){ minkowski(){ cube([s-2*r, s-2*r, s-2*r]); sphere(r=r, $fn=24); } }
module fente_aero(l, larg, n=6){ for(i=[0:n-1]) translate([0, i*(larg*2), 0]) cube([l, larg, wall+2]); }

// -------------------- CORPS DU CUBE ----------------------------------------
module cube_body(){
  difference(){
    // coque externe arrondie, ouverte en haut (capot separe)
    intersection(){ rcube(outer, fillet); cube([outer, outer, outer]); }
    // evidement interieur (ouvert en +Z)
    translate([wall, wall, wall]) cube([inner, inner, inner + 1]);

    // FENETRE ECRAN (face avant +Y), lamage 1 mm pour affleurer la dalle
    translate([outer/2, wall + 0.01, outer*0.55]) rotate([90,0,0]){
      translate([0,0,-wall-1]) cube([scr_win+clr, scr_win+clr, wall+2], center=true);
      translate([0,0,-wall+0.5]) cube([scr_pcb+1, scr_pcb+1, 1.0], center=true);   // lamage
    }
    // GRILLE HP (face arriere -Y)
    translate([outer*0.5, outer - wall - 0.01, outer*0.30]) rotate([90,0,0])
      for(a=[0:30:330], r=[hp_grid_d*0.18 : hp_grid_d*0.18 : hp_grid_d*0.5])
        translate([cos(a)*r, sin(a)*r, 0]) cylinder(h=wall+2, d=2.2, center=true);

    // axe du levier (face droite +X)
    translate([outer - wall - 0.01, outer/2, outer*0.5]) rotate([0,90,0])
      cylinder(h=wall+2, d=3.4, center=true);
    // passage USB-C (alim Pi) face gauche -X
    translate([-1, outer/2-5.5, wall+3]) cube([wall+2, 11, 4]);
    // bouton poussoir lateral Ø6.6 (reset)
    translate([outer+1, outer/2, outer*0.30]) rotate([0,-90,0]) cylinder(h=wall+2, d=6.6);
    // aerations fond (sous le SoC) + cheminee laterale haute (convection)
    translate([outer/2-15, outer/2-9, -1]) fente_aero(30, 1.4);
    translate([outer-wall-1, outer/2-15, inner*0.62]) rotate([0,90,0]) fente_aero(10, 1.4, 4);
  }

  // --- colonnes Pi Zero 2W (4x), carte centree, entraxe reel 58 x 23 ---
  px = (inner - pi_l)/2 + wall;
  py = (inner - pi_w)/2 + wall;
  for(dx=[0,pi_hole_dx], dy=[0,pi_hole_dy])
    translate([px + (pi_l-pi_hole_dx)/2 + dx, py + (pi_w-pi_hole_dy)/2 + dy, wall])
      difference(){ cylinder(h=10, d=post); translate([0,0,10-ins_h]) cylinder(h=ins_h+1, d=ins_d); }

  // --- berceau IMU : bloc RIGIDE, COPLANAIRE au fond => axes // faces (CRITIQUE) ---
  translate([outer/2 - acc_w/2, outer/2 - acc_l/2, wall]){
    difference(){
      cube([acc_w+2, acc_l+2, acc_seat_h+2]);                       // socle plein = rigide
      translate([1,1,2]) cube([acc_w+clr, acc_l+clr, acc_seat_h+2]); // logement carte
    }
    for(sx=[0,acc_hole_dx])                                         // 2 picots M2 anti-rotation
      translate([1+(acc_w-acc_hole_dx)/2+sx, acc_l/2+1, 0]) cylinder(h=3, d=1.8);
  }

  // --- paroi de separation batterie (compartiment thermique isole) ---
  translate([wall, inner - batt_w + wall, wall]) cube([2, batt_w, inner*0.5]);

  // --- anneau haut-parleur (face arriere) ---
  translate([outer/2, outer - wall - 1, outer*0.30]) rotate([90,0,0])
    difference(){ cylinder(h=3, d=hp_d+4); cylinder(h=3, d=hp_d+clr); }
}

// -------------------- CAPOT (face superieure, clipse) ----------------------
module lid_flat(){
  difference(){
    cube([outer, outer, wall]);
    translate([wall+clr, wall+clr, -0.01]) cube([inner-2*clr, inner-2*clr, wall*0.5]);
  }
  // jupe d'emboitement
  translate([wall+clr, wall+clr, -wall*0.6])
    difference(){
      cube([inner-2*clr, inner-2*clr, wall*0.6]);
      translate([1,1,-1]) cube([inner-2*clr-2, inner-2*clr-2, wall*0.6+2]);
    }
}

// -------------------- LEVIER MINECRAFT (piece separee) ---------------------
module lever(){
  axle_d = 3.0; base = 6;
  cylinder(h=4, d=axle_d, center=true);                          // axe 3mm
  hull(){ cylinder(h=4, d=7, center=true);
          translate([0,18,0]) cylinder(h=4, d=9, center=true); } // bras + pommeau
}

// -------------------- ASSEMBLAGE -------------------------------------------
module xerkion_cube(){ cube_body(); }

// rendu : decommenter UNE piece a imprimer
xerkion_cube();
// translate([0, outer+12, 0]) lid_flat();
// translate([outer+15, 0, 0]) lever();
