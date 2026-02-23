use anyhow::Result;
use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event};
use quick_xml::Writer;
use std::io::Cursor;

use super::model::*;

// ─── NIST Material Database ─────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize)]
pub struct NistMaterial {
    pub name: &'static str,
    pub density: f64,
    pub state: &'static str,
    pub category: &'static str,
}

pub fn find_nist_material(name: &str) -> Option<&'static NistMaterial> {
    NIST_MATERIALS.iter().find(|m| m.name == name)
}

pub fn search_nist_materials(query: &str, category: Option<&str>) -> Vec<&'static NistMaterial> {
    let q = query.to_uppercase();
    NIST_MATERIALS
        .iter()
        .filter(|m| {
            if let Some(cat) = category {
                if m.category != cat {
                    return false;
                }
            }
            if q.is_empty() {
                return true;
            }
            m.name.to_uppercase().contains(&q)
        })
        .collect()
}

pub static NIST_MATERIALS: &[NistMaterial] = &[
    // ── Elemental Materials (99) ──
    NistMaterial { name: "G4_WATER", density: 1.0, state: "Liquid", category: "Elemental" },
    NistMaterial { name: "G4_H", density: 8.375e-05, state: "Gas", category: "Elemental" },
    NistMaterial { name: "G4_He", density: 1.663e-04, state: "Gas", category: "Elemental" },
    NistMaterial { name: "G4_Li", density: 0.534, state: "Solid", category: "Elemental" },
    NistMaterial { name: "G4_Be", density: 1.848, state: "Solid", category: "Elemental" },
    NistMaterial { name: "G4_B", density: 2.37, state: "Solid", category: "Elemental" },
    NistMaterial { name: "G4_C", density: 2.0, state: "Solid", category: "Elemental" },
    NistMaterial { name: "G4_N", density: 1.165e-03, state: "Gas", category: "Elemental" },
    NistMaterial { name: "G4_O", density: 1.332e-03, state: "Gas", category: "Elemental" },
    NistMaterial { name: "G4_F", density: 1.580e-03, state: "Gas", category: "Elemental" },
    NistMaterial { name: "G4_Ne", density: 8.385e-04, state: "Gas", category: "Elemental" },
    NistMaterial { name: "G4_Na", density: 0.971, state: "Solid", category: "Elemental" },
    NistMaterial { name: "G4_Mg", density: 1.74, state: "Solid", category: "Elemental" },
    NistMaterial { name: "G4_Al", density: 2.699, state: "Solid", category: "Elemental" },
    NistMaterial { name: "G4_Si", density: 2.33, state: "Solid", category: "Elemental" },
    NistMaterial { name: "G4_P", density: 2.2, state: "Solid", category: "Elemental" },
    NistMaterial { name: "G4_S", density: 2.0, state: "Solid", category: "Elemental" },
    NistMaterial { name: "G4_Cl", density: 2.995e-03, state: "Gas", category: "Elemental" },
    NistMaterial { name: "G4_Ar", density: 1.662e-03, state: "Gas", category: "Elemental" },
    NistMaterial { name: "G4_K", density: 0.862, state: "Solid", category: "Elemental" },
    NistMaterial { name: "G4_Ca", density: 1.55, state: "Solid", category: "Elemental" },
    NistMaterial { name: "G4_Sc", density: 2.989, state: "Solid", category: "Elemental" },
    NistMaterial { name: "G4_Ti", density: 4.54, state: "Solid", category: "Elemental" },
    NistMaterial { name: "G4_V", density: 6.11, state: "Solid", category: "Elemental" },
    NistMaterial { name: "G4_Cr", density: 7.18, state: "Solid", category: "Elemental" },
    NistMaterial { name: "G4_Mn", density: 7.44, state: "Solid", category: "Elemental" },
    NistMaterial { name: "G4_Fe", density: 7.874, state: "Solid", category: "Elemental" },
    NistMaterial { name: "G4_Co", density: 8.9, state: "Solid", category: "Elemental" },
    NistMaterial { name: "G4_Ni", density: 8.902, state: "Solid", category: "Elemental" },
    NistMaterial { name: "G4_Cu", density: 8.96, state: "Solid", category: "Elemental" },
    NistMaterial { name: "G4_Zn", density: 7.133, state: "Solid", category: "Elemental" },
    NistMaterial { name: "G4_Ga", density: 5.904, state: "Solid", category: "Elemental" },
    NistMaterial { name: "G4_Ge", density: 5.323, state: "Solid", category: "Elemental" },
    NistMaterial { name: "G4_As", density: 5.73, state: "Solid", category: "Elemental" },
    NistMaterial { name: "G4_Se", density: 4.5, state: "Solid", category: "Elemental" },
    NistMaterial { name: "G4_Br", density: 7.072e-03, state: "Gas", category: "Elemental" },
    NistMaterial { name: "G4_Kr", density: 3.478e-03, state: "Gas", category: "Elemental" },
    NistMaterial { name: "G4_Rb", density: 1.532, state: "Solid", category: "Elemental" },
    NistMaterial { name: "G4_Sr", density: 2.54, state: "Solid", category: "Elemental" },
    NistMaterial { name: "G4_Y", density: 4.469, state: "Solid", category: "Elemental" },
    NistMaterial { name: "G4_Zr", density: 6.506, state: "Solid", category: "Elemental" },
    NistMaterial { name: "G4_Nb", density: 8.57, state: "Solid", category: "Elemental" },
    NistMaterial { name: "G4_Mo", density: 10.22, state: "Solid", category: "Elemental" },
    NistMaterial { name: "G4_Tc", density: 11.50, state: "Solid", category: "Elemental" },
    NistMaterial { name: "G4_Ru", density: 12.41, state: "Solid", category: "Elemental" },
    NistMaterial { name: "G4_Rh", density: 12.41, state: "Solid", category: "Elemental" },
    NistMaterial { name: "G4_Pd", density: 12.02, state: "Solid", category: "Elemental" },
    NistMaterial { name: "G4_Ag", density: 10.5, state: "Solid", category: "Elemental" },
    NistMaterial { name: "G4_Cd", density: 8.65, state: "Solid", category: "Elemental" },
    NistMaterial { name: "G4_In", density: 7.31, state: "Solid", category: "Elemental" },
    NistMaterial { name: "G4_Sn", density: 7.31, state: "Solid", category: "Elemental" },
    NistMaterial { name: "G4_Sb", density: 6.691, state: "Solid", category: "Elemental" },
    NistMaterial { name: "G4_Te", density: 6.24, state: "Solid", category: "Elemental" },
    NistMaterial { name: "G4_I", density: 4.93, state: "Solid", category: "Elemental" },
    NistMaterial { name: "G4_Xe", density: 5.485e-03, state: "Gas", category: "Elemental" },
    NistMaterial { name: "G4_Cs", density: 1.873, state: "Solid", category: "Elemental" },
    NistMaterial { name: "G4_Ba", density: 3.5, state: "Solid", category: "Elemental" },
    NistMaterial { name: "G4_La", density: 6.154, state: "Solid", category: "Elemental" },
    NistMaterial { name: "G4_Ce", density: 6.657, state: "Solid", category: "Elemental" },
    NistMaterial { name: "G4_Pr", density: 6.71, state: "Solid", category: "Elemental" },
    NistMaterial { name: "G4_Nd", density: 6.9, state: "Solid", category: "Elemental" },
    NistMaterial { name: "G4_Pm", density: 7.22, state: "Solid", category: "Elemental" },
    NistMaterial { name: "G4_Sm", density: 7.46, state: "Solid", category: "Elemental" },
    NistMaterial { name: "G4_Eu", density: 5.243, state: "Solid", category: "Elemental" },
    NistMaterial { name: "G4_Gd", density: 7.9004, state: "Solid", category: "Elemental" },
    NistMaterial { name: "G4_Tb", density: 8.229, state: "Solid", category: "Elemental" },
    NistMaterial { name: "G4_Dy", density: 8.55, state: "Solid", category: "Elemental" },
    NistMaterial { name: "G4_Ho", density: 8.795, state: "Solid", category: "Elemental" },
    NistMaterial { name: "G4_Er", density: 9.066, state: "Solid", category: "Elemental" },
    NistMaterial { name: "G4_Tm", density: 9.321, state: "Solid", category: "Elemental" },
    NistMaterial { name: "G4_Yb", density: 6.73, state: "Solid", category: "Elemental" },
    NistMaterial { name: "G4_Lu", density: 9.84, state: "Solid", category: "Elemental" },
    NistMaterial { name: "G4_Hf", density: 13.31, state: "Solid", category: "Elemental" },
    NistMaterial { name: "G4_Ta", density: 16.654, state: "Solid", category: "Elemental" },
    NistMaterial { name: "G4_W", density: 19.30, state: "Solid", category: "Elemental" },
    NistMaterial { name: "G4_Re", density: 21.02, state: "Solid", category: "Elemental" },
    NistMaterial { name: "G4_Os", density: 22.57, state: "Solid", category: "Elemental" },
    NistMaterial { name: "G4_Ir", density: 22.42, state: "Solid", category: "Elemental" },
    NistMaterial { name: "G4_Pt", density: 21.45, state: "Solid", category: "Elemental" },
    NistMaterial { name: "G4_Au", density: 19.32, state: "Solid", category: "Elemental" },
    NistMaterial { name: "G4_Hg", density: 13.546, state: "Solid", category: "Elemental" },
    NistMaterial { name: "G4_Tl", density: 11.72, state: "Solid", category: "Elemental" },
    NistMaterial { name: "G4_Pb", density: 11.35, state: "Solid", category: "Elemental" },
    NistMaterial { name: "G4_Bi", density: 9.747, state: "Solid", category: "Elemental" },
    NistMaterial { name: "G4_Po", density: 9.32, state: "Solid", category: "Elemental" },
    NistMaterial { name: "G4_At", density: 9.32, state: "Solid", category: "Elemental" },
    NistMaterial { name: "G4_Rn", density: 9.007e-03, state: "Gas", category: "Elemental" },
    NistMaterial { name: "G4_Fr", density: 1.00, state: "Solid", category: "Elemental" },
    NistMaterial { name: "G4_Ra", density: 5.00, state: "Solid", category: "Elemental" },
    NistMaterial { name: "G4_Ac", density: 10.07, state: "Solid", category: "Elemental" },
    NistMaterial { name: "G4_Th", density: 11.72, state: "Solid", category: "Elemental" },
    NistMaterial { name: "G4_Pa", density: 15.37, state: "Solid", category: "Elemental" },
    NistMaterial { name: "G4_U", density: 18.95, state: "Solid", category: "Elemental" },
    NistMaterial { name: "G4_Np", density: 20.25, state: "Solid", category: "Elemental" },
    NistMaterial { name: "G4_Pu", density: 19.84, state: "Solid", category: "Elemental" },
    NistMaterial { name: "G4_Am", density: 13.67, state: "Solid", category: "Elemental" },
    NistMaterial { name: "G4_Cm", density: 13.51, state: "Solid", category: "Elemental" },
    NistMaterial { name: "G4_Bk", density: 14.00, state: "Solid", category: "Elemental" },
    NistMaterial { name: "G4_Cf", density: 10.00, state: "Solid", category: "Elemental" },
    // ── Compound Materials (179) ──
    NistMaterial { name: "G4_A-150_TISSUE", density: 1.127, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_ACETONE", density: 0.7899, state: "Liquid", category: "Compound" },
    NistMaterial { name: "G4_ACETYLENE", density: 0.001097, state: "Gas", category: "Compound" },
    NistMaterial { name: "G4_ADENINE", density: 1.35, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_ADIPOSE_TISSUE_ICRP", density: 0.95, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_AIR", density: 0.001205, state: "Gas", category: "Compound" },
    NistMaterial { name: "G4_ALANINE", density: 1.42, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_ALUMINUM_OXIDE", density: 3.97, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_AMBER", density: 1.1, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_AMMONIA", density: 0.000826, state: "Gas", category: "Compound" },
    NistMaterial { name: "G4_ANILINE", density: 1.0235, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_ANTHRACENE", density: 1.283, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_B-100_BONE", density: 1.45, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_BAKELITE", density: 1.25, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_BARIUM_FLUORIDE", density: 4.89, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_BARIUM_SULFATE", density: 4.5, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_BENZENE", density: 0.87865, state: "Liquid", category: "Compound" },
    NistMaterial { name: "G4_BERYLLIUM_OXIDE", density: 3.01, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_BGO", density: 7.13, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_BLOOD_ICRP", density: 1.06, state: "Liquid", category: "Compound" },
    NistMaterial { name: "G4_BONE_COMPACT_ICRU", density: 1.85, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_BONE_CORTICAL_ICRP", density: 1.92, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_BORON_CARBIDE", density: 2.52, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_BORON_OXIDE", density: 1.812, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_BRAIN_ICRP", density: 1.04, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_BUTANE", density: 0.002493, state: "Gas", category: "Compound" },
    NistMaterial { name: "G4_N-BUTYL_ALCOHOL", density: 0.8098, state: "Liquid", category: "Compound" },
    NistMaterial { name: "G4_C-552", density: 1.76, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_CADMIUM_TELLURIDE", density: 6.2, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_CADMIUM_TUNGSTATE", density: 7.9, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_CALCIUM_CARBONATE", density: 2.8, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_CALCIUM_FLUORIDE", density: 3.18, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_CALCIUM_OXIDE", density: 3.3, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_CALCIUM_SULFATE", density: 2.96, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_CALCIUM_TUNGSTATE", density: 6.062, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_CARBON_DIOXIDE", density: 0.001842, state: "Gas", category: "Compound" },
    NistMaterial { name: "G4_CARBON_TETRACHLORIDE", density: 1.594, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_CELLULOSE_CELLOPHANE", density: 1.42, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_CELLULOSE_BUTYRATE", density: 1.2, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_CELLULOSE_NITRATE", density: 1.49, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_CERIC_SULFATE", density: 1.03, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_CESIUM_FLUORIDE", density: 4.115, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_CESIUM_IODIDE", density: 4.51, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_CHLOROBENZENE", density: 1.1058, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_CHLOROFORM", density: 1.4832, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_CONCRETE", density: 2.3, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_CYCLOHEXANE", density: 0.779, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_1,2-DICHLOROBENZENE", density: 1.3048, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_DICHLORODIETHYL_ETHER", density: 1.2199, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_1,2-DICHLOROETHANE", density: 1.2351, state: "Liquid", category: "Compound" },
    NistMaterial { name: "G4_DIETHYL_ETHER", density: 0.71378, state: "Liquid", category: "Compound" },
    NistMaterial { name: "G4_N,N-DIMETHYL_FORMAMIDE", density: 0.9487, state: "Liquid", category: "Compound" },
    NistMaterial { name: "G4_DIMETHYL_SULFOXIDE", density: 1.1014, state: "Liquid", category: "Compound" },
    NistMaterial { name: "G4_ETHANE", density: 0.001253, state: "Gas", category: "Compound" },
    NistMaterial { name: "G4_ETHYL_ALCOHOL", density: 0.7893, state: "Liquid", category: "Compound" },
    NistMaterial { name: "G4_ETHYL_CELLULOSE", density: 1.13, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_ETHYLENE", density: 0.001175, state: "Gas", category: "Compound" },
    NistMaterial { name: "G4_EYE_LENS_ICRP", density: 1.07, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_FERRIC_OXIDE", density: 5.2, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_FERROBORIDE", density: 7.15, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_FERROUS_OXIDE", density: 5.7, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_FERROUS_SULFATE", density: 1.024, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_FREON-12", density: 1.12, state: "Liquid", category: "Compound" },
    NistMaterial { name: "G4_FREON-12B2", density: 1.8, state: "Liquid", category: "Compound" },
    NistMaterial { name: "G4_FREON-13", density: 0.95, state: "Liquid", category: "Compound" },
    NistMaterial { name: "G4_FREON-13B1", density: 1.5, state: "Liquid", category: "Compound" },
    NistMaterial { name: "G4_FREON-13I1", density: 1.8, state: "Liquid", category: "Compound" },
    NistMaterial { name: "G4_GADOLINIUM_OXYSULFIDE", density: 7.44, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_GALLIUM_ARSENIDE", density: 5.31, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_GEL_PHOTO_EMULSION", density: 1.2914, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_Pyrex_Glass", density: 2.23, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_GLASS_LEAD", density: 6.22, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_GLASS_PLATE", density: 2.4, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_GLUTAMINE", density: 1.46, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_GLYCEROL", density: 1.2613, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_GUANINE", density: 1.58, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_GYPSUM", density: 2.32, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_N-HEPTANE", density: 0.68376, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_N-HEXANE", density: 0.6603, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_KAPTON", density: 1.42, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_LANTHANUM_OXYBROMIDE", density: 6.28, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_LANTHANUM_OXYSULFIDE", density: 5.86, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_LEAD_OXIDE", density: 9.53, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_LITHIUM_AMIDE", density: 1.178, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_LITHIUM_CARBONATE", density: 2.11, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_LITHIUM_FLUORIDE", density: 2.635, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_LITHIUM_HYDRIDE", density: 0.82, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_LITHIUM_IODIDE", density: 3.494, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_LITHIUM_OXIDE", density: 2.013, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_LITHIUM_TETRABORATE", density: 2.44, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_LUNG_ICRP", density: 1.04, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_M3_WAX", density: 1.05, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_MAGNESIUM_CARBONATE", density: 2.958, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_MAGNESIUM_FLUORIDE", density: 3.0, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_MAGNESIUM_OXIDE", density: 3.58, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_MAGNESIUM_TETRABORATE", density: 2.53, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_MERCURIC_IODIDE", density: 6.36, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_METHANE", density: 0.000667, state: "Gas", category: "Compound" },
    NistMaterial { name: "G4_METHANOL", density: 0.7914, state: "Liquid", category: "Compound" },
    NistMaterial { name: "G4_MIX_D_WAX", density: 0.99, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_MS20_TISSUE", density: 1.0, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_MUSCLE_SKELETAL_ICRP", density: 1.05, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_MUSCLE_STRIATED_ICRU", density: 1.04, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_MUSCLE_WITH_SUCROSE", density: 1.11, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_MUSCLE_WITHOUT_SUCROSE", density: 1.07, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_NAPHTHALENE", density: 1.145, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_NITROBENZENE", density: 1.19867, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_NITROUS_OXIDE", density: 0.001831, state: "Gas", category: "Compound" },
    NistMaterial { name: "G4_NYLON-8062", density: 1.08, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_NYLON-6-6", density: 1.14, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_NYLON-6-10", density: 1.14, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_NYLON-11_RILSAN", density: 1.425, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_OCTANE", density: 0.7026, state: "Liquid", category: "Compound" },
    NistMaterial { name: "G4_PARAFFIN", density: 0.93, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_N-PENTANE", density: 0.6262, state: "Liquid", category: "Compound" },
    NistMaterial { name: "G4_PHOTO_EMULSION", density: 3.815, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_PLASTIC_SC_VINYLTOLUENE", density: 1.032, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_PLUTONIUM_DIOXIDE", density: 11.46, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_POLYACRYLONITRILE", density: 1.17, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_POLYCARBONATE", density: 1.2, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_POLYCHLOROSTYRENE", density: 1.3, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_POLYETHYLENE", density: 0.94, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_MYLAR", density: 1.4, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_PLEXIGLASS", density: 1.19, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_POLYOXYMETHYLENE", density: 1.425, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_POLYPROPYLENE", density: 0.9, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_POLYSTYRENE", density: 1.06, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_TEFLON", density: 2.2, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_POLYTRIFLUOROCHLOROETHYLENE", density: 2.1, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_POLYVINYL_ACETATE", density: 1.19, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_POLYVINYL_ALCOHOL", density: 1.3, state: "Liquid", category: "Compound" },
    NistMaterial { name: "G4_POLYVINYL_BUTYRAL", density: 1.12, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_POLYVINYL_CHLORIDE", density: 1.3, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_POLYVINYLIDENE_CHLORIDE", density: 1.7, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_POLYVINYLIDENE_FLUORIDE", density: 1.76, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_POLYVINYL_PYRROLIDONE", density: 1.25, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_POTASSIUM_IODIDE", density: 3.13, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_POTASSIUM_OXIDE", density: 2.32, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_PROPANE", density: 0.001879, state: "Gas", category: "Compound" },
    NistMaterial { name: "G4_lPROPANE", density: 0.43, state: "Liquid", category: "Compound" },
    NistMaterial { name: "G4_N-PROPYL_ALCOHOL", density: 0.8035, state: "Liquid", category: "Compound" },
    NistMaterial { name: "G4_PYRIDINE", density: 0.9819, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_RUBBER_BUTYL", density: 0.92, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_RUBBER_NATURAL", density: 0.92, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_RUBBER_NEOPRENE", density: 1.23, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_SILICON_DIOXIDE", density: 2.32, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_SILVER_BROMIDE", density: 6.473, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_SILVER_CHLORIDE", density: 5.56, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_SILVER_HALIDES", density: 6.47, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_SILVER_IODIDE", density: 6.01, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_SKIN_ICRP", density: 1.09, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_SODIUM_CARBONATE", density: 2.532, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_SODIUM_IODIDE", density: 3.667, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_SODIUM_MONOXIDE", density: 2.27, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_SODIUM_NITRATE", density: 2.261, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_STILBENE", density: 0.9707, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_SUCROSE", density: 1.5805, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_TERPHENYL", density: 1.24, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_TESTIS_ICRP", density: 1.04, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_TETRACHLOROETHYLENE", density: 1.625, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_THALLIUM_CHLORIDE", density: 7.004, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_TISSUE_SOFT_ICRP", density: 1.03, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_TISSUE_SOFT_ICRU-4", density: 1.0, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_TISSUE-METHANE", density: 0.001064, state: "Gas", category: "Compound" },
    NistMaterial { name: "G4_TISSUE-PROPANE", density: 0.001826, state: "Gas", category: "Compound" },
    NistMaterial { name: "G4_TITANIUM_DIOXIDE", density: 4.26, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_TOLUENE", density: 0.8669, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_TRICHLOROETHYLENE", density: 1.46, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_TRIETHYL_PHOSPHATE", density: 1.07, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_TUNGSTEN_HEXAFLUORIDE", density: 2.4, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_URANIUM_DICARBIDE", density: 11.28, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_URANIUM_MONOCARBIDE", density: 13.63, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_URANIUM_OXIDE", density: 10.96, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_UREA", density: 1.323, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_VALINE", density: 1.23, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_VITON", density: 1.8, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_WATER_VAPOR", density: 0.000756, state: "Gas", category: "Compound" },
    NistMaterial { name: "G4_XYLENE", density: 0.87, state: "Solid", category: "Compound" },
    NistMaterial { name: "G4_GRAPHITE", density: 2.21, state: "Solid", category: "Compound" },
    // ── HEP and Nuclear Materials (16) ──
    NistMaterial { name: "G4_lH2", density: 0.0708, state: "Liquid", category: "HEP" },
    NistMaterial { name: "G4_lN2", density: 0.807, state: "Liquid", category: "HEP" },
    NistMaterial { name: "G4_lO2", density: 1.141, state: "Liquid", category: "HEP" },
    NistMaterial { name: "G4_lAr", density: 1.396, state: "Liquid", category: "HEP" },
    NistMaterial { name: "G4_lBr", density: 3.1028, state: "Liquid", category: "HEP" },
    NistMaterial { name: "G4_lKr", density: 2.418, state: "Liquid", category: "HEP" },
    NistMaterial { name: "G4_lXe", density: 2.953, state: "Liquid", category: "HEP" },
    NistMaterial { name: "G4_PbWO4", density: 8.28, state: "Solid", category: "HEP" },
    NistMaterial { name: "G4_Galactic", density: 1e-25, state: "Gas", category: "HEP" },
    NistMaterial { name: "G4_GRAPHITE_POROUS", density: 1.7, state: "Solid", category: "HEP" },
    NistMaterial { name: "G4_LUCITE", density: 1.19, state: "Solid", category: "HEP" },
    NistMaterial { name: "G4_BRASS", density: 8.52, state: "Solid", category: "HEP" },
    NistMaterial { name: "G4_BRONZE", density: 8.82, state: "Solid", category: "HEP" },
    NistMaterial { name: "G4_STAINLESS-STEEL", density: 8.00, state: "Solid", category: "HEP" },
    NistMaterial { name: "G4_CR39", density: 1.32, state: "Solid", category: "HEP" },
    NistMaterial { name: "G4_OCTADECANOL", density: 0.812, state: "Solid", category: "HEP" },
    // ── Space (ISS) Materials (3) ──
    NistMaterial { name: "G4_KEVLAR", density: 1.44, state: "Solid", category: "Space" },
    NistMaterial { name: "G4_DACRON", density: 1.40, state: "Solid", category: "Space" },
    NistMaterial { name: "G4_NEOPRENE", density: 1.23, state: "Solid", category: "Space" },
    // ── Biochemical / DNA Materials (12) ──
    NistMaterial { name: "G4_CYTOSINE", density: 1.3, state: "Solid", category: "Biochemical" },
    NistMaterial { name: "G4_THYMINE", density: 1.48, state: "Solid", category: "Biochemical" },
    NistMaterial { name: "G4_URACIL", density: 1.32, state: "Solid", category: "Biochemical" },
    NistMaterial { name: "G4_DEOXYRIBOSE", density: 1.5, state: "Solid", category: "Biochemical" },
    NistMaterial { name: "G4_PHOSPHORIC_ACID", density: 1.87, state: "Solid", category: "Biochemical" },
    NistMaterial { name: "G4_DNA_DEOXYRIBOSE", density: 1.0, state: "Solid", category: "Biochemical" },
    NistMaterial { name: "G4_DNA_PHOSPHATE", density: 1.0, state: "Solid", category: "Biochemical" },
    NistMaterial { name: "G4_DNA_ADENINE", density: 1.0, state: "Solid", category: "Biochemical" },
    NistMaterial { name: "G4_DNA_GUANINE", density: 1.0, state: "Solid", category: "Biochemical" },
    NistMaterial { name: "G4_DNA_CYTOSINE", density: 1.0, state: "Solid", category: "Biochemical" },
    NistMaterial { name: "G4_DNA_THYMINE", density: 1.0, state: "Solid", category: "Biochemical" },
    NistMaterial { name: "G4_DNA_URACIL", density: 1.0, state: "Solid", category: "Biochemical" },
];

// ─── GDML Serializer ────────────────────────────────────────────────────────

pub fn serialize_gdml(doc: &GdmlDocument) -> Result<String> {
    let mut writer = Writer::new_with_indent(Cursor::new(Vec::new()), b' ', 2);

    // XML declaration
    writer.write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))?;

    // <gdml> root with namespace
    let mut gdml = BytesStart::new("gdml");
    gdml.push_attribute(("xmlns:xsi", "http://www.w3.org/2001/XMLSchema-instance"));
    gdml.push_attribute(("xsi:noNamespaceSchemaLocation", "http://service-spi.web.cern.ch/service-spi/app/releases/GDML/schema/gdml.xsd"));
    writer.write_event(Event::Start(gdml))?;

    write_defines(&mut writer, &doc.defines)?;
    write_materials(&mut writer, &doc.materials)?;
    write_solids(&mut writer, &doc.solids)?;
    write_structure(&mut writer, &doc.structure)?;
    write_setup(&mut writer, &doc.setup)?;

    // </gdml>
    writer.write_event(Event::End(BytesEnd::new("gdml")))?;

    let result = writer.into_inner().into_inner();
    Ok(String::from_utf8(result)?)
}

fn write_defines(writer: &mut Writer<Cursor<Vec<u8>>>, defines: &DefineSection) -> Result<()> {
    writer.write_event(Event::Start(BytesStart::new("define")))?;

    for c in &defines.constants {
        let mut elem = BytesStart::new("constant");
        elem.push_attribute(("name", c.name.as_str()));
        elem.push_attribute(("value", c.value.as_str()));
        writer.write_event(Event::Empty(elem))?;
    }

    for q in &defines.quantities {
        let mut elem = BytesStart::new("quantity");
        elem.push_attribute(("name", q.name.as_str()));
        if let Some(ref t) = q.r#type {
            elem.push_attribute(("type", t.as_str()));
        }
        elem.push_attribute(("value", q.value.as_str()));
        if let Some(ref u) = q.unit {
            elem.push_attribute(("unit", u.as_str()));
        }
        writer.write_event(Event::Empty(elem))?;
    }

    for v in &defines.variables {
        let mut elem = BytesStart::new("variable");
        elem.push_attribute(("name", v.name.as_str()));
        elem.push_attribute(("value", v.value.as_str()));
        writer.write_event(Event::Empty(elem))?;
    }

    for e in &defines.expressions {
        let mut elem = BytesStart::new("expression");
        elem.push_attribute(("name", e.name.as_str()));
        writer.write_event(Event::Start(elem))?;
        writer.write_event(Event::Text(BytesText::new(&e.value)))?;
        writer.write_event(Event::End(BytesEnd::new("expression")))?;
    }

    for p in &defines.positions {
        let mut elem = BytesStart::new("position");
        elem.push_attribute(("name", p.name.as_str()));
        if let Some(ref x) = p.x {
            elem.push_attribute(("x", x.as_str()));
        }
        if let Some(ref y) = p.y {
            elem.push_attribute(("y", y.as_str()));
        }
        if let Some(ref z) = p.z {
            elem.push_attribute(("z", z.as_str()));
        }
        if let Some(ref u) = p.unit {
            elem.push_attribute(("unit", u.as_str()));
        }
        writer.write_event(Event::Empty(elem))?;
    }

    for r in &defines.rotations {
        let mut elem = BytesStart::new("rotation");
        elem.push_attribute(("name", r.name.as_str()));
        if let Some(ref x) = r.x {
            elem.push_attribute(("x", x.as_str()));
        }
        if let Some(ref y) = r.y {
            elem.push_attribute(("y", y.as_str()));
        }
        if let Some(ref z) = r.z {
            elem.push_attribute(("z", z.as_str()));
        }
        if let Some(ref u) = r.unit {
            elem.push_attribute(("unit", u.as_str()));
        }
        writer.write_event(Event::Empty(elem))?;
    }

    writer.write_event(Event::End(BytesEnd::new("define")))?;
    Ok(())
}

fn write_materials(writer: &mut Writer<Cursor<Vec<u8>>>, materials: &MaterialSection) -> Result<()> {
    writer.write_event(Event::Start(BytesStart::new("materials")))?;

    for el in &materials.elements {
        let mut elem = BytesStart::new("element");
        elem.push_attribute(("name", el.name.as_str()));
        if let Some(ref f) = el.formula {
            elem.push_attribute(("formula", f.as_str()));
        }
        if let Some(ref z) = el.z {
            elem.push_attribute(("Z", z.as_str()));
        }
        if let Some(ref av) = el.atom_value {
            writer.write_event(Event::Start(elem))?;
            let mut atom = BytesStart::new("atom");
            atom.push_attribute(("value", av.as_str()));
            writer.write_event(Event::Empty(atom))?;
            writer.write_event(Event::End(BytesEnd::new("element")))?;
        } else {
            writer.write_event(Event::Empty(elem))?;
        }
    }

    for mat in &materials.materials {
        let mut elem = BytesStart::new("material");
        elem.push_attribute(("name", mat.name.as_str()));
        if let Some(ref f) = mat.formula {
            elem.push_attribute(("formula", f.as_str()));
        }
        if let Some(ref z) = mat.z {
            elem.push_attribute(("Z", z.as_str()));
        }
        writer.write_event(Event::Start(elem))?;

        if let Some(ref d) = mat.density {
            let mut de = BytesStart::new("D");
            de.push_attribute(("value", d.value.as_str()));
            if let Some(ref u) = d.unit {
                de.push_attribute(("unit", u.as_str()));
            }
            writer.write_event(Event::Empty(de))?;
        }
        if let Some(ref dr) = mat.density_ref {
            let mut dref = BytesStart::new("Dref");
            dref.push_attribute(("ref", dr.as_str()));
            writer.write_event(Event::Empty(dref))?;
        }
        if let Some(ref t) = mat.temperature {
            let mut te = BytesStart::new("T");
            te.push_attribute(("value", t.value.as_str()));
            if let Some(ref u) = t.unit {
                te.push_attribute(("unit", u.as_str()));
            }
            writer.write_event(Event::Empty(te))?;
        }
        if let Some(ref p) = mat.pressure {
            let mut pe = BytesStart::new("P");
            pe.push_attribute(("value", p.value.as_str()));
            if let Some(ref u) = p.unit {
                pe.push_attribute(("unit", u.as_str()));
            }
            writer.write_event(Event::Empty(pe))?;
        }
        if let Some(ref av) = mat.atom_value {
            let mut atom = BytesStart::new("atom");
            atom.push_attribute(("value", av.as_str()));
            writer.write_event(Event::Empty(atom))?;
        }
        for comp in &mat.components {
            match comp {
                MaterialComponent::Fraction { n, ref_name } => {
                    let mut fe = BytesStart::new("fraction");
                    fe.push_attribute(("n", n.as_str()));
                    fe.push_attribute(("ref", ref_name.as_str()));
                    writer.write_event(Event::Empty(fe))?;
                }
                MaterialComponent::Composite { n, ref_name } => {
                    let mut ce = BytesStart::new("composite");
                    ce.push_attribute(("n", n.as_str()));
                    ce.push_attribute(("ref", ref_name.as_str()));
                    writer.write_event(Event::Empty(ce))?;
                }
            }
        }

        writer.write_event(Event::End(BytesEnd::new("material")))?;
    }

    writer.write_event(Event::End(BytesEnd::new("materials")))?;
    Ok(())
}

fn write_solids(writer: &mut Writer<Cursor<Vec<u8>>>, solids: &SolidSection) -> Result<()> {
    writer.write_event(Event::Start(BytesStart::new("solids")))?;

    for solid in &solids.solids {
        match solid {
            Solid::Box(b) => {
                let mut elem = BytesStart::new("box");
                elem.push_attribute(("name", b.name.as_str()));
                elem.push_attribute(("x", b.x.as_str()));
                elem.push_attribute(("y", b.y.as_str()));
                elem.push_attribute(("z", b.z.as_str()));
                if let Some(ref u) = b.lunit {
                    elem.push_attribute(("lunit", u.as_str()));
                }
                writer.write_event(Event::Empty(elem))?;
            }
            Solid::Tube(t) => {
                let mut elem = BytesStart::new("tube");
                elem.push_attribute(("name", t.name.as_str()));
                if let Some(ref v) = t.rmin {
                    elem.push_attribute(("rmin", v.as_str()));
                }
                elem.push_attribute(("rmax", t.rmax.as_str()));
                elem.push_attribute(("z", t.z.as_str()));
                if let Some(ref v) = t.startphi {
                    elem.push_attribute(("startphi", v.as_str()));
                }
                if let Some(ref v) = t.deltaphi {
                    elem.push_attribute(("deltaphi", v.as_str()));
                }
                if let Some(ref u) = t.aunit {
                    elem.push_attribute(("aunit", u.as_str()));
                }
                if let Some(ref u) = t.lunit {
                    elem.push_attribute(("lunit", u.as_str()));
                }
                writer.write_event(Event::Empty(elem))?;
            }
            Solid::Cone(c) => {
                let mut elem = BytesStart::new("cone");
                elem.push_attribute(("name", c.name.as_str()));
                if let Some(ref v) = c.rmin1 {
                    elem.push_attribute(("rmin1", v.as_str()));
                }
                elem.push_attribute(("rmax1", c.rmax1.as_str()));
                if let Some(ref v) = c.rmin2 {
                    elem.push_attribute(("rmin2", v.as_str()));
                }
                elem.push_attribute(("rmax2", c.rmax2.as_str()));
                elem.push_attribute(("z", c.z.as_str()));
                if let Some(ref v) = c.startphi {
                    elem.push_attribute(("startphi", v.as_str()));
                }
                if let Some(ref v) = c.deltaphi {
                    elem.push_attribute(("deltaphi", v.as_str()));
                }
                if let Some(ref u) = c.aunit {
                    elem.push_attribute(("aunit", u.as_str()));
                }
                if let Some(ref u) = c.lunit {
                    elem.push_attribute(("lunit", u.as_str()));
                }
                writer.write_event(Event::Empty(elem))?;
            }
            Solid::Sphere(s) => {
                let mut elem = BytesStart::new("sphere");
                elem.push_attribute(("name", s.name.as_str()));
                if let Some(ref v) = s.rmin {
                    elem.push_attribute(("rmin", v.as_str()));
                }
                elem.push_attribute(("rmax", s.rmax.as_str()));
                if let Some(ref v) = s.startphi {
                    elem.push_attribute(("startphi", v.as_str()));
                }
                if let Some(ref v) = s.deltaphi {
                    elem.push_attribute(("deltaphi", v.as_str()));
                }
                if let Some(ref v) = s.starttheta {
                    elem.push_attribute(("starttheta", v.as_str()));
                }
                if let Some(ref v) = s.deltatheta {
                    elem.push_attribute(("deltatheta", v.as_str()));
                }
                if let Some(ref u) = s.aunit {
                    elem.push_attribute(("aunit", u.as_str()));
                }
                if let Some(ref u) = s.lunit {
                    elem.push_attribute(("lunit", u.as_str()));
                }
                writer.write_event(Event::Empty(elem))?;
            }
        }
    }

    writer.write_event(Event::End(BytesEnd::new("solids")))?;
    Ok(())
}

fn write_structure(writer: &mut Writer<Cursor<Vec<u8>>>, structure: &StructureSection) -> Result<()> {
    writer.write_event(Event::Start(BytesStart::new("structure")))?;

    for vol in &structure.volumes {
        let mut elem = BytesStart::new("volume");
        elem.push_attribute(("name", vol.name.as_str()));
        writer.write_event(Event::Start(elem))?;

        let mut mref = BytesStart::new("materialref");
        mref.push_attribute(("ref", vol.material_ref.as_str()));
        writer.write_event(Event::Empty(mref))?;

        let mut sref = BytesStart::new("solidref");
        sref.push_attribute(("ref", vol.solid_ref.as_str()));
        writer.write_event(Event::Empty(sref))?;

        for pv in &vol.physvols {
            let mut pv_elem = BytesStart::new("physvol");
            if let Some(ref n) = pv.name {
                pv_elem.push_attribute(("name", n.as_str()));
            }
            writer.write_event(Event::Start(pv_elem))?;

            if let Some(ref fref) = pv.file_ref {
                let mut fe = BytesStart::new("file");
                fe.push_attribute(("name", fref.name.as_str()));
                if let Some(ref vn) = fref.volname {
                    fe.push_attribute(("volname", vn.as_str()));
                }
                writer.write_event(Event::Empty(fe))?;
            } else {
                let mut vref = BytesStart::new("volumeref");
                vref.push_attribute(("ref", pv.volume_ref.as_str()));
                writer.write_event(Event::Empty(vref))?;
            }

            match &pv.position {
                Some(PlacementPos::Inline(p)) => {
                    let mut pe = BytesStart::new("position");
                    if !p.name.is_empty() {
                        pe.push_attribute(("name", p.name.as_str()));
                    }
                    if let Some(ref x) = p.x {
                        pe.push_attribute(("x", x.as_str()));
                    }
                    if let Some(ref y) = p.y {
                        pe.push_attribute(("y", y.as_str()));
                    }
                    if let Some(ref z) = p.z {
                        pe.push_attribute(("z", z.as_str()));
                    }
                    if let Some(ref u) = p.unit {
                        pe.push_attribute(("unit", u.as_str()));
                    }
                    writer.write_event(Event::Empty(pe))?;
                }
                Some(PlacementPos::Ref(name)) => {
                    let mut pr = BytesStart::new("positionref");
                    pr.push_attribute(("ref", name.as_str()));
                    writer.write_event(Event::Empty(pr))?;
                }
                None => {}
            }

            match &pv.rotation {
                Some(PlacementRot::Inline(r)) => {
                    let mut re = BytesStart::new("rotation");
                    if !r.name.is_empty() {
                        re.push_attribute(("name", r.name.as_str()));
                    }
                    if let Some(ref x) = r.x {
                        re.push_attribute(("x", x.as_str()));
                    }
                    if let Some(ref y) = r.y {
                        re.push_attribute(("y", y.as_str()));
                    }
                    if let Some(ref z) = r.z {
                        re.push_attribute(("z", z.as_str()));
                    }
                    if let Some(ref u) = r.unit {
                        re.push_attribute(("unit", u.as_str()));
                    }
                    writer.write_event(Event::Empty(re))?;
                }
                Some(PlacementRot::Ref(name)) => {
                    let mut rr = BytesStart::new("rotationref");
                    rr.push_attribute(("ref", name.as_str()));
                    writer.write_event(Event::Empty(rr))?;
                }
                None => {}
            }

            writer.write_event(Event::End(BytesEnd::new("physvol")))?;
        }

        for aux in &vol.auxiliaries {
            let mut ae = BytesStart::new("auxiliary");
            ae.push_attribute(("auxtype", aux.auxtype.as_str()));
            ae.push_attribute(("auxvalue", aux.auxvalue.as_str()));
            writer.write_event(Event::Empty(ae))?;
        }

        writer.write_event(Event::End(BytesEnd::new("volume")))?;
    }

    writer.write_event(Event::End(BytesEnd::new("structure")))?;
    Ok(())
}

fn write_setup(writer: &mut Writer<Cursor<Vec<u8>>>, setup: &SetupSection) -> Result<()> {
    let mut elem = BytesStart::new("setup");
    elem.push_attribute(("name", setup.name.as_str()));
    elem.push_attribute(("version", setup.version.as_str()));
    writer.write_event(Event::Start(elem))?;

    let mut world = BytesStart::new("world");
    world.push_attribute(("ref", setup.world_ref.as_str()));
    writer.write_event(Event::Empty(world))?;

    writer.write_event(Event::End(BytesEnd::new("setup")))?;
    Ok(())
}
