//! Stable product taxonomy built on top of the official Places365 leaf labels.
//!
//! Places365 remains the model taxonomy. The complete base mapping is kept
//! private, while the public clusters are the versioned product taxonomy so a
//! topic keeps the same meaning across libraries and analysis runs.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SceneCluster {
    pub id: &'static str,
    pub display_name: &'static str,
    pub leaf_labels: &'static [&'static str],
}

pub const TAXONOMY_VERSION: &str = "photo-organizer-photography-topics-v1";
pub const PLACES365_LEAF_COUNT: usize = 365;

/// The official Places365 scene groups are kept as an implementation detail.
/// They provide a complete, auditable base mapping for all 365 model labels.
const PLACES365_BASE_CLUSTERS: [SceneCluster; 10] = [
    SceneCluster {
        id: "scene_nature",
        display_name: "自然风景与地貌",
        leaf_labels: &[
            "badlands",
            "bamboo_forest",
            "beach",
            "butte",
            "canal/natural",
            "canyon",
            "cliff",
            "coast",
            "creek",
            "crevasse",
            "desert/sand",
            "desert/vegetation",
            "desert_road",
            "field/wild",
            "forest/broadleaf",
            "forest_path",
            "forest_road",
            "glacier",
            "grotto",
            "hot_spring",
            "ice_floe",
            "ice_shelf",
            "iceberg",
            "islet",
            "lagoon",
            "lake/natural",
            "marsh",
            "moat/water",
            "mountain",
            "mountain_path",
            "mountain_snowy",
            "ocean",
            "pond",
            "rainforest",
            "river",
            "rock_arch",
            "sky",
            "snowfield",
            "swamp",
            "swimming_hole",
            "tundra",
            "underwater/ocean_deep",
            "valley",
            "volcano",
            "waterfall",
            "watering_hole",
            "wave",
        ],
    },
    SceneCluster {
        id: "scene_urban",
        display_name: "城市街道与社区",
        leaf_labels: &[
            "alley",
            "canal/urban",
            "crosswalk",
            "downtown",
            "driveway",
            "phone_booth",
            "plaza",
            "promenade",
            "residential_neighborhood",
            "slum",
            "street",
            "village",
        ],
    },
    SceneCluster {
        id: "scene_architecture",
        display_name: "建筑、地标与宗教",
        leaf_labels: &[
            "aqueduct",
            "arch",
            "archaelogical_excavation",
            "bridge",
            "building_facade",
            "burial_chamber",
            "castle",
            "catacomb",
            "cemetery",
            "church/indoor",
            "church/outdoor",
            "courthouse",
            "courtyard",
            "doorway/outdoor",
            "embassy",
            "fire_escape",
            "kasbah",
            "lighthouse",
            "medina",
            "mausoleum",
            "mosque/outdoor",
            "museum/outdoor",
            "office_building",
            "pagoda",
            "palace",
            "ruin",
            "rope_bridge",
            "skyscraper",
            "synagogue/outdoor",
            "temple/asia",
            "throne_room",
            "tower",
            "viaduct",
        ],
    },
    SceneCluster {
        id: "scene_commerce",
        display_name: "餐饮与商业",
        leaf_labels: &[
            "auto_showroom",
            "bakery/shop",
            "bar",
            "banquet_hall",
            "bazaar/indoor",
            "bazaar/outdoor",
            "beauty_salon",
            "beer_garden",
            "beer_hall",
            "bookstore",
            "booth/indoor",
            "butchers_shop",
            "cafeteria",
            "candy_store",
            "clothing_store",
            "coffee_shop",
            "delicatessen",
            "department_store",
            "diner/outdoor",
            "dining_hall",
            "drugstore",
            "escalator/indoor",
            "fabric_store",
            "fastfood_restaurant",
            "flea_market/indoor",
            "florist_shop/indoor",
            "food_court",
            "general_store/indoor",
            "general_store/outdoor",
            "gift_shop",
            "hardware_store",
            "ice_cream_parlor",
            "jewelry_shop",
            "laundromat",
            "market/indoor",
            "market/outdoor",
            "pet_shop",
            "pharmacy",
            "pizzeria",
            "pub/indoor",
            "restaurant",
            "restaurant_kitchen",
            "restaurant_patio",
            "shoe_shop",
            "shopfront",
            "shopping_mall/indoor",
            "supermarket",
            "sushi_bar",
            "toyshop",
        ],
    },
    SceneCluster {
        id: "scene_residential",
        display_name: "居住与生活空间",
        leaf_labels: &[
            "alcove",
            "apartment_building/outdoor",
            "artists_loft",
            "attic",
            "balcony/exterior",
            "balcony/interior",
            "basement",
            "bathroom",
            "beach_house",
            "bedchamber",
            "bedroom",
            "bow_window/indoor",
            "cabin/outdoor",
            "chalet",
            "childs_room",
            "closet",
            "cottage",
            "dining_room",
            "dorm_room",
            "dressing_room",
            "home_office",
            "home_theater",
            "house",
            "hunting_lodge/outdoor",
            "igloo",
            "jacuzzi/indoor",
            "kitchen",
            "living_room",
            "mansion",
            "manufactured_home",
            "mezzanine",
            "pantry",
            "patio",
            "playroom",
            "porch",
            "sauna",
            "shower",
            "staircase",
            "storage_room",
            "television_room",
            "utility_room",
            "wet_bar",
            "yard",
        ],
    },
    SceneCluster {
        id: "scene_public",
        display_name: "工作、教育、医疗与公共室内",
        leaf_labels: &[
            "archive",
            "art_gallery",
            "art_school",
            "art_studio",
            "atrium/public",
            "bank_vault",
            "biology_laboratory",
            "campus",
            "chemistry_lab",
            "classroom",
            "clean_room",
            "computer_room",
            "conference_center",
            "conference_room",
            "corridor",
            "elevator/door",
            "elevator_lobby",
            "elevator_shaft",
            "entrance_hall",
            "fire_station",
            "hospital",
            "hospital_room",
            "jail_cell",
            "kindergarden_classroom",
            "lecture_room",
            "legislative_chamber",
            "library/indoor",
            "library/outdoor",
            "lobby",
            "museum/indoor",
            "natural_history_museum",
            "nursing_home",
            "office",
            "office_cubicles",
            "operating_room",
            "physics_laboratory",
            "reception",
            "schoolhouse",
            "science_museum",
            "server_room",
            "television_studio",
            "veterinarians_office",
            "waiting_room",
        ],
    },
    SceneCluster {
        id: "scene_transport",
        display_name: "交通、旅行与交通设施",
        leaf_labels: &[
            "airfield",
            "airplane_cabin",
            "airport_terminal",
            "berth",
            "boat_deck",
            "boathouse",
            "bus_interior",
            "bus_station/indoor",
            "car_interior",
            "cockpit",
            "galley",
            "garage/indoor",
            "garage/outdoor",
            "gas_station",
            "hangar/indoor",
            "hangar/outdoor",
            "harbor",
            "heliport",
            "highway",
            "hotel/outdoor",
            "hotel_room",
            "inn/outdoor",
            "landing_deck",
            "motel",
            "parking_garage/indoor",
            "parking_garage/outdoor",
            "parking_lot",
            "pier",
            "raft",
            "railroad_track",
            "runway",
            "subway_station/platform",
            "ticket_booth",
            "train_interior",
            "train_station/platform",
            "youth_hostel",
        ],
    },
    SceneCluster {
        id: "scene_sports",
        display_name: "运动、娱乐与活动",
        leaf_labels: &[
            "amphitheater",
            "amusement_arcade",
            "amusement_park",
            "aquarium",
            "arcade",
            "arena/hockey",
            "arena/performance",
            "arena/rodeo",
            "athletic_field/outdoor",
            "auditorium",
            "ball_pit",
            "ballroom",
            "baseball_field",
            "basketball_court/indoor",
            "bowling_alley",
            "boxing_ring",
            "bullring",
            "carrousel",
            "discotheque",
            "football_field",
            "golf_course",
            "gymnasium/indoor",
            "ice_skating_rink/indoor",
            "ice_skating_rink/outdoor",
            "locker_room",
            "martial_arts_gym",
            "movie_theater/indoor",
            "music_studio",
            "orchestra_pit",
            "playground",
            "racecourse",
            "raceway",
            "recreation_room",
            "sandbox",
            "ski_resort",
            "ski_slope",
            "soccer_field",
            "stadium/baseball",
            "stadium/football",
            "stadium/soccer",
            "stage/indoor",
            "stage/outdoor",
            "swimming_pool/indoor",
            "swimming_pool/outdoor",
            "volleyball_court/outdoor",
            "water_park",
        ],
    },
    SceneCluster {
        id: "scene_industrial",
        display_name: "工业、施工、能源与军事",
        leaf_labels: &[
            "army_base",
            "assembly_line",
            "auto_factory",
            "construction_site",
            "dam",
            "engine_room",
            "excavation",
            "industrial_area",
            "junkyard",
            "landfill",
            "loading_dock",
            "lock_chamber",
            "oilrig",
            "repair_shop",
            "trench",
            "water_tower",
            "wind_farm",
        ],
    },
    SceneCluster {
        id: "scene_agriculture",
        display_name: "农业、园林与户外休闲",
        leaf_labels: &[
            "barn",
            "barndoor",
            "boardwalk",
            "botanical_garden",
            "campsite",
            "corn_field",
            "corral",
            "farm",
            "field/cultivated",
            "field_road",
            "fishpond",
            "formal_garden",
            "fountain",
            "gazebo/exterior",
            "greenhouse/indoor",
            "greenhouse/outdoor",
            "hayfield",
            "japanese_garden",
            "kennel/outdoor",
            "lawn",
            "nursery",
            "oast_house",
            "orchard",
            "park",
            "pasture",
            "picnic_area",
            "pavilion",
            "rice_paddy",
            "roof_garden",
            "shed",
            "stable",
            "topiary_garden",
            "tree_farm",
            "tree_house",
            "vegetable_garden",
            "vineyard",
            "wheat_field",
            "windmill",
            "zen_garden",
        ],
    },
];

/// Product-facing topics are deliberately written for a photographer's
/// workflow rather than for a scene-recognition benchmark.  The leaf labels
/// are mapped through `photo_cluster_id_for_leaf` below so that one Places365
/// label can be redirected from a broad scene bucket into a useful topic such
/// as food, travel, or plants.
pub const SCENE_CLUSTERS: [SceneCluster; 11] = [
    SceneCluster {
        id: "photo_landscape",
        display_name: "风光自然",
        leaf_labels: &[],
    },
    SceneCluster {
        id: "photo_urban",
        display_name: "城市街拍",
        leaf_labels: &[],
    },
    SceneCluster {
        id: "photo_architecture",
        display_name: "建筑与空间",
        leaf_labels: &[],
    },
    SceneCluster {
        id: "photo_food",
        display_name: "美食餐饮",
        leaf_labels: &[],
    },
    SceneCluster {
        id: "photo_commercial",
        display_name: "商业与静物",
        leaf_labels: &[],
    },
    SceneCluster {
        id: "photo_indoor",
        display_name: "室内与生活",
        leaf_labels: &[],
    },
    SceneCluster {
        id: "photo_travel",
        display_name: "旅行人文",
        leaf_labels: &[],
    },
    SceneCluster {
        id: "photo_event",
        display_name: "活动与运动",
        leaf_labels: &[],
    },
    SceneCluster {
        id: "photo_transport",
        display_name: "交通与汽车",
        leaf_labels: &[],
    },
    SceneCluster {
        id: "photo_plant",
        display_name: "植物与园艺",
        leaf_labels: &[],
    },
    SceneCluster {
        id: "photo_documentary",
        display_name: "纪实与工业",
        leaf_labels: &[],
    },
];

const PHOTO_FOOD_LEAVES: &[&str] = &[
    "bakery/shop",
    "bar",
    "banquet_hall",
    "beer_garden",
    "beer_hall",
    "butchers_shop",
    "cafeteria",
    "candy_store",
    "coffee_shop",
    "delicatessen",
    "diner/outdoor",
    "dining_hall",
    "fastfood_restaurant",
    "food_court",
    "ice_cream_parlor",
    "pizzeria",
    "pub/indoor",
    "restaurant",
    "restaurant_kitchen",
    "restaurant_patio",
    "sushi_bar",
];

const PHOTO_TRAVEL_LEAVES: &[&str] = &[
    "airfield",
    "airplane_cabin",
    "airport_terminal",
    "art_gallery",
    "art_school",
    "art_studio",
    "berth",
    "boat_deck",
    "boathouse",
    "bus_station/indoor",
    "cockpit",
    "galley",
    "harbor",
    "heliport",
    "hotel/outdoor",
    "hotel_room",
    "inn/outdoor",
    "landing_deck",
    "museum/indoor",
    "natural_history_museum",
    "motel",
    "pier",
    "raft",
    "runway",
    "science_museum",
    "subway_station/platform",
    "ticket_booth",
    "train_interior",
    "train_station/platform",
    "youth_hostel",
];

const PHOTO_PLANT_LEAVES: &[&str] = &[
    "botanical_garden",
    "corn_field",
    "field/cultivated",
    "formal_garden",
    "greenhouse/indoor",
    "greenhouse/outdoor",
    "hayfield",
    "japanese_garden",
    "lawn",
    "nursery",
    "orchard",
    "pasture",
    "rice_paddy",
    "roof_garden",
    "topiary_garden",
    "tree_farm",
    "vegetable_garden",
    "vineyard",
    "wheat_field",
    "zen_garden",
];

fn base_cluster_id_for_leaf(label: &str) -> Option<&'static str> {
    PLACES365_BASE_CLUSTERS
        .iter()
        .find(|cluster| cluster.leaf_labels.contains(&label))
        .map(|cluster| cluster.id)
}

/// Returns the stable photography topic for an official Places365 leaf.
///
/// Places365 is a scene model, so this is intentionally described as a topic
/// inference. It must not be presented as subject recognition (for example,
/// it cannot reliably tell whether a scene contains a person or a portrait).
pub fn photo_cluster_id_for_leaf(label: &str) -> Option<&'static str> {
    if PHOTO_FOOD_LEAVES.contains(&label) {
        return Some("photo_food");
    }
    if PHOTO_TRAVEL_LEAVES.contains(&label) {
        return Some("photo_travel");
    }
    if PHOTO_PLANT_LEAVES.contains(&label) {
        return Some("photo_plant");
    }

    match base_cluster_id_for_leaf(label)? {
        "scene_nature" => Some("photo_landscape"),
        "scene_urban" => Some("photo_urban"),
        "scene_architecture" => Some("photo_architecture"),
        "scene_commerce" => Some("photo_commercial"),
        "scene_residential" => Some("photo_indoor"),
        "scene_public" => Some("photo_indoor"),
        "scene_transport" => Some("photo_transport"),
        "scene_sports" => Some("photo_event"),
        "scene_industrial" => Some("photo_documentary"),
        "scene_agriculture" => Some("photo_landscape"),
        _ => None,
    }
}

pub fn scene_cluster_index_for_leaf(label: &str) -> Option<usize> {
    let cluster_id = photo_cluster_id_for_leaf(label)?;
    SCENE_CLUSTERS
        .iter()
        .position(|cluster| cluster.id == cluster_id)
}

pub fn scene_cluster_for_leaf(label: &str) -> Option<&'static SceneCluster> {
    scene_cluster_index_for_leaf(label).and_then(|index| SCENE_CLUSTERS.get(index))
}

pub fn scene_cluster_id_for_leaf(label: &str) -> Option<&'static str> {
    scene_cluster_for_leaf(label).map(|cluster| cluster.id)
}

pub fn scene_cluster_display_name(id: &str) -> Option<&'static str> {
    SCENE_CLUSTERS
        .iter()
        .find(|cluster| cluster.id == id)
        .map(|cluster| cluster.display_name)
}

pub fn all_leaf_labels() -> impl Iterator<Item = &'static str> {
    PLACES365_BASE_CLUSTERS
        .iter()
        .flat_map(|cluster| cluster.leaf_labels.iter().copied())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mapping_covers_all_places365_leaves_exactly_once() {
        let mut labels = all_leaf_labels().collect::<Vec<_>>();
        assert_eq!(labels.len(), PLACES365_LEAF_COUNT);
        labels.sort_unstable();
        labels.dedup();
        assert_eq!(labels.len(), PLACES365_LEAF_COUNT);
        assert_eq!(
            PLACES365_BASE_CLUSTERS
                .iter()
                .map(|cluster| cluster.leaf_labels.len())
                .sum::<usize>(),
            PLACES365_LEAF_COUNT
        );
        assert!(all_leaf_labels().all(|label| scene_cluster_for_leaf(label).is_some()));
    }

    #[test]
    fn every_photography_topic_has_mapped_evidence() {
        let mut counts = vec![0usize; SCENE_CLUSTERS.len()];
        for label in all_leaf_labels() {
            counts[scene_cluster_index_for_leaf(label).expect("leaf must have a topic")] += 1;
        }
        assert!(counts.iter().all(|count| *count > 0));
    }

    #[test]
    fn boundary_labels_are_in_the_intended_cluster() {
        assert_eq!(
            scene_cluster_id_for_leaf("restaurant_patio"),
            Some("photo_food")
        );
        assert_eq!(
            scene_cluster_id_for_leaf("apartment_building/outdoor"),
            Some("photo_indoor")
        );
        assert_eq!(
            scene_cluster_id_for_leaf("museum/outdoor"),
            Some("photo_architecture")
        );
        assert_eq!(
            scene_cluster_id_for_leaf("museum/indoor"),
            Some("photo_travel")
        );
        assert_eq!(
            scene_cluster_id_for_leaf("highway"),
            Some("photo_transport")
        );
        assert_eq!(scene_cluster_id_for_leaf("street"), Some("photo_urban"));
        assert_eq!(
            scene_cluster_id_for_leaf("medina"),
            Some("photo_architecture")
        );
        assert_eq!(
            scene_cluster_id_for_leaf("shopfront"),
            Some("photo_commercial")
        );
        assert_eq!(
            scene_cluster_id_for_leaf("formal_garden"),
            Some("photo_plant")
        );
        assert_eq!(
            scene_cluster_id_for_leaf("field/wild"),
            Some("photo_landscape")
        );
    }
}
