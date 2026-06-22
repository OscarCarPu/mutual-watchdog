// ===== WATCHDOG CASE =====
// Set which part to render
show_body = 1;  // 1 = show body, 0 = hide
show_lid = 0;   // 1 = show lid, 0 = hide



// ===== LID =====
lid_plug_height = 5;     // How far lid plugs into the case
lid_top_height = 3;      // Thickness of lid top
lid_tolerance = 0.3;     // Gap so lid fits (0.3-0.5 is typical for FDM printing)

// ===== INTERIOR DIMENSIONS =====
interior_width = 25;     // 22mm holder + 1.5mm wiggle each side
interior_depth = 25;     // same
interior_height = 84 + lid_plug_height;    // 76mm holder + 6mm ESP + 2mm wiggle

// ===== WALL THICKNESS =====
wall = 2;
bottom = 2;

// ===== OUTER DIMENSIONS =====
outer_width = interior_width + (wall * 2);    // 29mm
outer_depth = interior_depth + (wall * 2);    // 29mm
outer_height = interior_height + bottom;      // 86mm

// ===== BODY =====
if (show_body == 1) {
    difference() {
        // Solid outer box
        cube([outer_width, outer_depth, outer_height]);
        
        // Hollow interior
        translate([wall, wall, bottom])
            cube([interior_width, interior_depth, interior_height + 1]);  // +1 to ensure clean cut at top
    }
}

// ===== LID =====
if (show_lid == 1) {
    // Top of lid (sits flush with case top)
    cube([outer_width, outer_depth, lid_top_height]);
    
    // Plug part (goes inside the case)
    translate([wall + lid_tolerance, wall + lid_tolerance, lid_top_height])
        cube([
            interior_width - (lid_tolerance * 2),
            interior_depth - (lid_tolerance * 2),
            lid_plug_height
        ]);
}