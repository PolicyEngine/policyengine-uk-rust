use std::collections::HashMap;
use std::path::Path;
use crate::engine::entities::*;
use crate::data::Dataset;
use crate::data::frs::{load_table_cols, get_f64, get_i64, region_from_gvtregno};

const WEEKS_IN_YEAR: f64 = 365.25 / 7.0;

/// All 275 LCFS product-level expenditure codes (COICOP individual items).
/// These are the base codes (no _c/_t/_w suffixes) from dvhh_ukanon.
const PRODUCT_CODES: &[&str] = &[
    "c11111","c11121","c11122","c11131","c11141","c11142","c11151",
    "c11211","c11221","c11231","c11241","c11251","c11252","c11253",
    "c11261","c11271","c11311","c11321","c11331","c11341",
    "c11411","c11421","c11431","c11441","c11451","c11461","c11471",
    "c11511","c11521","c11522","c11531","c11541","c11551",
    "c11611","c11621","c11631","c11641","c11651","c11661","c11671","c11681",
    "c11691",
    "c11711","c11721","c11731","c11741","c11751","c11761","c11771","c11781",
    "c11811","c11821","c11831","c11841","c11851","c11861",
    "c11911","c11921","c11931","c11941",
    "c12111","c12121","c12131","c12211","c12221","c12231","c12241",
    "c21111","c21211","c21212","c21213","c21214","c21221","c21311",
    "c22111","c22121","c22131",
    "c31111","c31211","c31212","c31221","c31222",
    "c31231","c31232","c31233","c31234",
    "c31311","c31312","c31313","c31314","c31315",
    "c31411","c31412","c31413",
    "c32111","c32121","c32131","c32211",
    "c41211","c43111","c43112","c44211",
    "c45112","c45114","c45212","c45214","c45222","c45312","c45411","c45412","c45511",
    "c51113","c51114","c51212","c51311",
    "c52111","c52112",
    "c53111","c53121","c53122","c53131","c53132","c53133","c53141","c53151","c53161","c53171",
    "c53211","c53311","c53312","c53313","c53314",
    "c54111","c54121","c54131","c54132","c54141",
    "c55111","c55112","c55211","c55212","c55213","c55214",
    "c56111","c56112","c56121","c56122","c56123","c56124","c56125",
    "c56211","c56221","c56222","c56223",
    "c61111","c61112","c61211","c61311","c61312","c61313",
    "c62111","c62112","c62113","c62114","c62211","c62212",
    "c62311","c62321","c62322","c62331","c63111",
    "c71112","c71122","c71212","c71311","c71411",
    "c72111","c72112","c72113","c72114","c72115",
    "c72211","c72212","c72213",
    "c72313","c72314","c72411","c72412","c72413","c72414",
    "c73112","c73212","c73213","c73214","c73311","c73312","c73411","c73512","c73513","c73611",
    "c81111","c82111","c82112","c82113","c83112","c83114","c83115",
    "c91111","c91112","c91113","c91121","c91122","c91123","c91124","c91125","c91126","c91127","c91128",
    "c91211","c91221","c91311",
    "c91411","c91412","c91413","c91414","c91511",
    "c92111","c92112","c92114","c92116","c92117",
    "c92211","c92221","c92311",
    "c93111","c93112","c93113","c93114","c93211","c93212",
    "c93311","c93312","c93313","c93411","c93412","c93511",
    "c94111","c94112","c94113","c94115",
    "c94211","c94212","c94221",
    "c94232","c94236","c94238","c94239",
    "c94241","c94242","c94243","c94244","c94245","c94246",
    "c94311","c94312","c94313","c94314","c94315","c94316","c94319",
    "c95111","c95211","c95212","c95311","c95411",
];

/// Map LCFS column codes to descriptive variable names for consumption_products.
/// Derived from LCFS Volume F derived variables documentation.
const PRODUCT_NAMES: &[(&str, &str)] = &[
    ("c11111", "rice"),
    ("c11121", "bread"),
    ("c11122", "buns_crispbread_and_biscuits"),
    ("c11131", "pasta_products"),
    ("c11141", "cakes_and_puddings"),
    ("c11142", "pastry_savoury"),
    ("c11151", "other_breads_and_cereals"),
    ("c11211", "beef_fresh_chilled_or_frozen"),
    ("c11221", "pork_fresh_chilled_or_frozen"),
    ("c11231", "lamb_fresh_chilled_or_frozen"),
    ("c11241", "poultry_fresh_chilled_or_frozen"),
    ("c11251", "sausages"),
    ("c11252", "bacon_and_ham"),
    ("c11253", "offal_pate_etc"),
    ("c11261", "other_preserved_or_processed_meat"),
    ("c11271", "other_fresh_chilled_or_frozen_edible_meat"),
    ("c11311", "fish_fresh_chilled_or_frozen"),
    ("c11321", "seafood_fresh_chilled_or_frozen"),
    ("c11331", "dried_smoked_or_salted_fish_and_seafood"),
    ("c11341", "other_preserved_or_processed_fish"),
    ("c11411", "whole_milk"),
    ("c11421", "low_fat_milk"),
    ("c11431", "preserved_milk"),
    ("c11441", "yoghurt"),
    ("c11451", "cheese_and_curd"),
    ("c11461", "other_milk_products"),
    ("c11471", "eggs"),
    ("c11511", "butter"),
    ("c11521", "margarine_and_other_vegetable_fats"),
    ("c11522", "peanut_butter"),
    ("c11531", "olive_oil"),
    ("c11541", "edible_oils"),
    ("c11551", "other_edible_animal_fats"),
    ("c11611", "citrus_fruits_fresh"),
    ("c11621", "bananas_fresh"),
    ("c11631", "apples_fresh"),
    ("c11641", "pears_fresh"),
    ("c11651", "stone_fruits_fresh"),
    ("c11661", "berries_fresh"),
    ("c11671", "other_fresh_chilled_or_frozen_fruits"),
    ("c11681", "dried_fruit_and_nuts"),
    ("c11691", "preserved_fruit_and_fruit_based_products"),
    ("c11711", "leaf_and_stem_vegetables"),
    ("c11721", "cabbages"),
    ("c11731", "vegetables_grown_for_fruit"),
    ("c11741", "root_crops_and_mushrooms"),
    ("c11751", "dried_vegetables"),
    ("c11761", "other_preserved_or_processed_vegetables"),
    ("c11771", "potatoes"),
    ("c11781", "other_tubers"),
    ("c11811", "sugar"),
    ("c11821", "jams_marmalades"),
    ("c11831", "chocolate"),
    ("c11841", "confectionery_products"),
    ("c11851", "edible_ices_and_ice_cream"),
    ("c11861", "other_sugar_products"),
    ("c11911", "sauces_condiments"),
    ("c11921", "salt_spices_and_culinary_herbs"),
    ("c11931", "yeast_dessert_preparations_soups"),
    ("c11941", "other_food_products"),
    ("c12111", "coffee"),
    ("c12121", "tea"),
    ("c12131", "cocoa_and_powdered_chocolate"),
    ("c12211", "mineral_or_spring_waters"),
    ("c12221", "soft_drinks"),
    ("c12231", "fruit_juices_and_squash"),
    ("c12241", "vegetable_juices"),
    ("c21111", "spirits_and_liqueurs"),
    ("c21211", "wine"),
    ("c21212", "fortified_wine"),
    ("c21213", "ciders_and_perry"),
    ("c21214", "alcopops"),
    ("c21221", "champagne_and_sparkling_wines"),
    ("c21311", "beer_and_lager"),
    ("c22111", "cigarettes"),
    ("c22121", "cigars"),
    ("c22131", "other_tobacco"),
    ("c31111", "clothing_materials"),
    ("c31211", "mens_outer_garments"),
    ("c31212", "mens_under_garments"),
    ("c31221", "womens_outer_garments"),
    ("c31222", "womens_under_garments"),
    ("c31231", "boys_outer_garments"),
    ("c31232", "girls_outer_garments"),
    ("c31233", "infants_outer_garments"),
    ("c31234", "childrens_under_garments"),
    ("c31311", "mens_accessories"),
    ("c31312", "womens_accessories"),
    ("c31313", "childrens_accessories"),
    ("c31314", "haberdashery"),
    ("c31315", "protective_headgear"),
    ("c31411", "clothing_hire"),
    ("c31412", "dry_cleaners_and_dyeing"),
    ("c31413", "laundry_laundrettes"),
    ("c32111", "footwear_for_men"),
    ("c32121", "footwear_for_women"),
    ("c32131", "footwear_for_children_and_infants"),
    ("c32211", "repair_and_hire_of_footwear"),
    ("c41211", "second_dwelling_rent"),
    ("c43111", "paint_wallpaper_timber"),
    ("c43112", "equipment_hire_small_materials"),
    ("c44211", "refuse_collection"),
    ("c45112", "second_dwelling_electricity"),
    ("c45114", "electricity_slot_meter"),
    ("c45212", "second_dwelling_gas"),
    ("c45214", "gas_slot_meter"),
    ("c45222", "bottled_gas"),
    ("c45312", "paraffin"),
    ("c45411", "coal_and_coke"),
    ("c45412", "wood_and_peat"),
    ("c45511", "hot_water_steam_and_ice"),
    ("c51113", "fancy_decorative_goods"),
    ("c51114", "garden_furniture"),
    ("c51212", "hard_floor_coverings"),
    ("c51311", "repair_of_furniture_furnishings"),
    ("c52111", "bedroom_textiles"),
    ("c52112", "other_household_textiles"),
    ("c53111", "refrigerators_freezers"),
    ("c53121", "washing_machines_dryers"),
    ("c53122", "dish_washing_machines"),
    ("c53131", "gas_cookers"),
    ("c53132", "electric_cookers"),
    ("c53133", "microwave_ovens"),
    ("c53141", "heaters_air_conditioners"),
    ("c53151", "vacuum_cleaners"),
    ("c53161", "sewing_and_knitting_machines"),
    ("c53171", "fire_extinguisher_safes_etc"),
    ("c53211", "small_electric_household_appliances"),
    ("c53311", "spare_parts_gas_and_electric_appliances"),
    ("c53312", "electrical_appliance_repairs"),
    ("c53313", "gas_appliance_repairs"),
    ("c53314", "rental_hire_of_major_household_appliance"),
    ("c54111", "glassware_china_pottery"),
    ("c54121", "cutlery_and_silverware"),
    ("c54131", "kitchen_utensils"),
    ("c54132", "storage_and_other_durable_household_articles"),
    ("c54141", "repair_of_glassware_tableware"),
    ("c55111", "electrical_tools"),
    ("c55112", "lawn_mowers"),
    ("c55211", "small_tools"),
    ("c55212", "door_electrical_and_other_fittings"),
    ("c55213", "garden_tools_and_equipment"),
    ("c55214", "electrical_consumables"),
    ("c56111", "detergents_washing_up_liquid"),
    ("c56112", "disinfectants_polishes_cleaning_materials"),
    ("c56121", "kitchen_disposables"),
    ("c56122", "household_hardware_and_appliances"),
    ("c56123", "kitchen_gloves_cloths_etc"),
    ("c56124", "pins_needles_and_tape_measures"),
    ("c56125", "nails_nuts_bolts_tape_and_glue"),
    ("c56211", "domestic_services"),
    ("c56221", "cleaning_of_carpets_curtains"),
    ("c56222", "other_household_services"),
    ("c56223", "hire_of_furniture_and_furnishings"),
    ("c61111", "nhs_prescription_charges"),
    ("c61112", "medicines_and_medical_goods"),
    ("c61211", "other_medical_products"),
    ("c61311", "spectacles_lenses"),
    ("c61312", "accessories_repairs_to_spectacles"),
    ("c61313", "non_optical_appliances_and_equipment"),
    ("c62111", "nhs_medical_services"),
    ("c62112", "private_medical_services"),
    ("c62113", "nhs_optical_services"),
    ("c62114", "private_optical_services"),
    ("c62211", "nhs_dental_services"),
    ("c62212", "private_dental_services"),
    ("c62311", "medical_analysis_laboratories"),
    ("c62321", "nhs_medical_auxiliaries"),
    ("c62322", "private_medical_auxiliaries"),
    ("c62331", "non_hospital_ambulance_services"),
    ("c63111", "hospital_services"),
    ("c71112", "loan_hp_new_car_van"),
    ("c71122", "loan_hp_secondhand_car_van"),
    ("c71212", "loan_hp_motorcycle"),
    ("c71311", "purchase_of_bicycle"),
    ("c71411", "animal_drawn_vehicles"),
    ("c72111", "car_van_accessories_and_fittings"),
    ("c72112", "car_van_spare_parts"),
    ("c72113", "motorcycle_accessories_and_spare_parts"),
    ("c72114", "antifreeze_cleaning_materials"),
    ("c72115", "bicycle_accessories_repairs"),
    ("c72211", "petrol"),
    ("c72212", "diesel_oil"),
    ("c72213", "other_motor_oils"),
    ("c72313", "motoring_organisation_subscription"),
    ("c72314", "car_washing_and_breakdown_services"),
    ("c72411", "parking_fees_tolls_and_permits"),
    ("c72412", "garage_rent_mot_etc"),
    ("c72413", "driving_lessons"),
    ("c72414", "hire_of_self_drive_cars_vans_bicycles"),
    ("c73112", "railway_and_tube_fares"),
    ("c73212", "bus_and_coach_fares"),
    ("c73213", "taxis_and_hired_cars"),
    ("c73214", "other_personal_travel"),
    ("c73311", "air_fares_within_uk"),
    ("c73312", "air_fares_international"),
    ("c73411", "water_travel"),
    ("c73512", "combined_fares"),
    ("c73513", "school_travel"),
    ("c73611", "delivery_charges"),
    ("c81111", "postage"),
    ("c82111", "telephone_purchase"),
    ("c82112", "mobile_phone_purchase"),
    ("c82113", "fax_machine_purchase"),
    ("c83112", "telephone_coin_and_other_payments"),
    ("c83114", "mobile_phone_other_payments"),
    ("c83115", "second_dwelling_telephone"),
    ("c91111", "audio_equipment"),
    ("c91112", "audio_equipment_in_car"),
    ("c91113", "audio_accessories"),
    ("c91121", "television_set"),
    ("c91122", "satellite_dish"),
    ("c91123", "satellite_dish_installation"),
    ("c91124", "video_recorder"),
    ("c91125", "digital_tv_decoder"),
    ("c91126", "spare_parts_for_tv_video_audio"),
    ("c91127", "cable_tv_connection"),
    ("c91128", "dvd_purchase"),
    ("c91211", "photographic_equipment"),
    ("c91221", "optical_instruments"),
    ("c91311", "personal_computers_printers"),
    ("c91411", "records_cds_audio_cassettes"),
    ("c91412", "video_cassettes"),
    ("c91413", "camera_films"),
    ("c91414", "dvds"),
    ("c91511", "repair_of_av_photo_equipment"),
    ("c92111", "boats_trailers_and_horses"),
    ("c92112", "caravans_mobile_homes"),
    ("c92114", "motor_caravan_new"),
    ("c92116", "motor_caravan_secondhand"),
    ("c92117", "accessories_for_boats_caravans"),
    ("c92211", "musical_instruments"),
    ("c92221", "major_durables_for_indoor_recreation"),
    ("c92311", "maintenance_repair_of_recreation_durables"),
    ("c93111", "games_toys_and_hobbies"),
    ("c93112", "computer_software_and_games"),
    ("c93113", "console_computer_games"),
    ("c93114", "games_toys_misc_decorative"),
    ("c93211", "equipment_for_sport_camping"),
    ("c93212", "bbq_and_swings"),
    ("c93311", "plants_flowers_seeds"),
    ("c93312", "garden_decorative"),
    ("c93313", "artificial_flowers"),
    ("c93411", "pet_food"),
    ("c93412", "pet_purchase_and_accessories"),
    ("c93511", "veterinary_services"),
    ("c94111", "spectator_sports"),
    ("c94112", "participant_sports"),
    ("c94113", "sports_and_social_club_subscriptions"),
    ("c94115", "hire_of_sport_equipment"),
    ("c94211", "cinemas"),
    ("c94212", "live_entertainment"),
    ("c94221", "museums_zoos_theme_parks"),
    ("c94232", "tv_licence_second_dwelling"),
    ("c94236", "tv_slot_meter_payments"),
    ("c94238", "video_cassette_rental"),
    ("c94239", "cassette_cd_hire"),
    ("c94241", "admissions_to_clubs_dances_bingo"),
    ("c94242", "social_events_and_gatherings"),
    ("c94243", "leisure_subscriptions"),
    ("c94244", "other_subscriptions"),
    ("c94245", "internet_subscription_fees"),
    ("c94246", "film_development_and_photos"),
    ("c94311", "football_pools_stakes"),
    ("c94312", "bingo_stakes"),
    ("c94313", "lottery_stakes"),
    ("c94314", "bookmaker_tote_betting_stakes"),
    ("c94315", "irish_lottery_stakes"),
    ("c94316", "national_lottery_instants"),
    ("c94319", "national_lottery_stakes"),
    ("c95111", "books"),
    ("c95211", "newspapers"),
    ("c95212", "magazines_and_periodicals"),
    ("c95311", "cards_calendars_posters"),
    ("c95411", "stationery_diaries_art_materials"),
];

/// Look up descriptive name for an LCFS product code, falling back to the code itself.
fn product_name(code: &str) -> &str {
    PRODUCT_NAMES.iter()
        .find(|(c, _)| *c == code)
        .map(|(_, name)| *name)
        .unwrap_or(code)
}

/// Parse Living Costs and Food Survey (LCFS) microdata from UKDS tab-delimited files.
///
/// The LCFS is a household + person survey covering consumption and expenditure.
/// Person-level income data is limited but sufficient for basic tax-benefit simulation.
///
/// Expected directory structure:
///   data_dir/lcfs_{YYYY}_dvhh_ukanon*.tab   (household derived variables)
///   data_dir/lcfs_{YYYY}_dvper_ukanon*.tab   (person derived variables)
///
/// LCFS income values are WEEKLY — we annualise by multiplying by WEEKS_IN_YEAR.
pub fn load_lcfs(data_dir: &Path, fiscal_year: u32) -> anyhow::Result<Dataset> {
    let (hh_file, person_file) = find_lcfs_files(data_dir, fiscal_year)?;

    // Build list of columns to load: core + top-level COICOP + all product codes
    let mut hh_cols: Vec<&str> = vec![
        "case", "gorx", "weighta",
        "p389p", "p344p",  // total expenditure, total income
        "g018", "g019",    // num adults, num children
        "a122",            // tenure type
        // COICOP top-level consumption categories (weekly)
        "p601", "p602", "p603", "p604", "p605", "p606",
        "p607", "p608", "p609", "p610", "p611", "p612",
        "c72211", "c72212",  // petrol, diesel (also in PRODUCT_CODES but fine to duplicate)
    ];
    for code in PRODUCT_CODES {
        if !hh_cols.contains(code) {
            hh_cols.push(code);
        }
    }

    // Load household table
    let hh_table = load_table_cols(data_dir, &hh_file, Some(&hh_cols))?;

    // Load person table
    let person_table = load_table_cols(data_dir, &person_file, Some(&[
        "case", "person",
        "a003", "a004", "a002",  // age (two variants), sex
        "b303p", "b3262p",       // employment income, self-employment income
        "b3381", "p049p",        // state pension, private pension income
    ]))?;

    // Group persons by household case number
    let mut persons_by_case: HashMap<i64, Vec<&HashMap<String, String>>> = HashMap::new();
    for row in &person_table {
        let case = get_i64(row, "case");
        persons_by_case.entry(case).or_default().push(row);
    }

    let mut people = Vec::new();
    let mut benunits = Vec::new();
    let mut households = Vec::new();

    for hh_row in &hh_table {
        let case = get_i64(hh_row, "case");
        let weight = get_f64(hh_row, "weighta");
        if weight <= 0.0 { continue; }

        let region = region_from_gvtregno(get_i64(hh_row, "gorx"));
        let hh_id = households.len();
        let bu_id = benunits.len();

        let mut hh_person_ids = Vec::new();

        // Build person records from person table if available
        if let Some(person_rows) = persons_by_case.get(&case) {
            for (i, prow) in person_rows.iter().enumerate() {
                let pid = people.len();
                hh_person_ids.push(pid);

                // Age: try a004 first (derived), fall back to a003
                let age = {
                    let a = get_f64(prow, "a004");
                    if a > 0.0 { a } else { get_f64(prow, "a003") }
                };

                let person = Person {
                    id: pid,
                    benunit_id: bu_id,
                    household_id: hh_id,
                    age,
                    gender: if get_i64(prow, "a002") == 1 { Gender::Male } else { Gender::Female },
                    is_benunit_head: i == 0,
                    is_household_head: i == 0,
                    is_in_scotland: region.is_scotland(),
                    // Income (weekly → annual)
                    employment_income: get_f64(prow, "b303p").max(0.0) * WEEKS_IN_YEAR,
                    self_employment_income: get_f64(prow, "b3262p").max(0.0) * WEEKS_IN_YEAR,
                    state_pension: get_f64(prow, "b3381").max(0.0) * WEEKS_IN_YEAR,
                    pension_income: get_f64(prow, "p049p").max(0.0) * WEEKS_IN_YEAR,
                    ..Person::default()
                };
                people.push(person);
            }
        } else {
            // No person records — create synthetic persons from household counts
            let num_adults = get_i64(hh_row, "g018").max(1) as usize;
            let num_children = get_i64(hh_row, "g019").max(0) as usize;

            for i in 0..(num_adults + num_children) {
                let pid = people.len();
                hh_person_ids.push(pid);
                let is_child = i >= num_adults;

                let person = Person {
                    id: pid,
                    benunit_id: bu_id,
                    household_id: hh_id,
                    age: if is_child { 8.0 } else { 40.0 },
                    gender: Gender::Male,
                    is_benunit_head: i == 0,
                    is_household_head: i == 0,
                    is_in_scotland: region.is_scotland(),
                    ..Person::default()
                };
                people.push(person);
            }
        }

        // If no persons were created at all, create one adult
        if hh_person_ids.is_empty() {
            let pid = people.len();
            hh_person_ids.push(pid);
            let person = Person {
                id: pid,
                benunit_id: bu_id,
                household_id: hh_id,
                age: 40.0,
                gender: Gender::Male,
                is_benunit_head: true,
                is_household_head: true,
                is_in_scotland: region.is_scotland(),
                ..Person::default()
            };
            people.push(person);
        }

        let benunit = BenUnit {
            id: bu_id,
            household_id: hh_id,
            person_ids: hh_person_ids.clone(),
            ..BenUnit::default()
        };
        benunits.push(benunit);

        // Product-level consumption (weekly → annual), keyed by descriptive name
        let mut consumption_products = HashMap::new();
        for &code in PRODUCT_CODES {
            let val = get_f64(hh_row, code).max(0.0) * WEEKS_IN_YEAR;
            if val > 0.0 {
                consumption_products.insert(product_name(code).to_string(), val);
            }
        }

        let household = Household {
            id: hh_id,
            benunit_ids: vec![bu_id],
            person_ids: hh_person_ids,
            weight,
            region,
            // COICOP consumption (weekly → annual)
            food_and_non_alcoholic_beverages: get_f64(hh_row, "p601").max(0.0) * WEEKS_IN_YEAR,
            alcohol_and_tobacco: get_f64(hh_row, "p602").max(0.0) * WEEKS_IN_YEAR,
            clothing_and_footwear: get_f64(hh_row, "p603").max(0.0) * WEEKS_IN_YEAR,
            housing_water_and_fuel: get_f64(hh_row, "p604").max(0.0) * WEEKS_IN_YEAR,
            household_furnishings: get_f64(hh_row, "p605").max(0.0) * WEEKS_IN_YEAR,
            health: get_f64(hh_row, "p606").max(0.0) * WEEKS_IN_YEAR,
            transport: get_f64(hh_row, "p607").max(0.0) * WEEKS_IN_YEAR,
            communication: get_f64(hh_row, "p608").max(0.0) * WEEKS_IN_YEAR,
            recreation_and_culture: get_f64(hh_row, "p609").max(0.0) * WEEKS_IN_YEAR,
            education: get_f64(hh_row, "p610").max(0.0) * WEEKS_IN_YEAR,
            restaurants_and_hotels: get_f64(hh_row, "p611").max(0.0) * WEEKS_IN_YEAR,
            miscellaneous_goods_and_services: get_f64(hh_row, "p612").max(0.0) * WEEKS_IN_YEAR,
            petrol_spending: get_f64(hh_row, "c72211").max(0.0) * WEEKS_IN_YEAR,
            diesel_spending: get_f64(hh_row, "c72212").max(0.0) * WEEKS_IN_YEAR,
            consumption_products,
            ..Household::default()
        };
        households.push(household);
    }

    Ok(Dataset {
        people,
        benunits,
        households,
        name: format!("Living Costs and Food Survey {}/{:02}", fiscal_year, (fiscal_year + 1) % 100),
        year: fiscal_year,
    })
}

/// Find LCFS tab file names in the directory.
/// LCFS file naming varies between years, e.g.:
///   lcfs_2021_dvhh_ukanon.tab
///   lcfs_2021_dvper_ukanon202122.tab
/// We search for files matching the pattern rather than hardcoding.
fn find_lcfs_files(data_dir: &Path, fiscal_year: u32) -> anyhow::Result<(String, String)> {
    let mut hh_file = None;
    let mut person_file = None;

    let entries = std::fs::read_dir(data_dir)?;
    for entry in entries {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_lowercase();

        if (name.contains("dvhh") || name.contains("dv_hh")) && (name.ends_with(".tab") || name.ends_with(".csv")) {
            // Strip extension for load_table_cols
            let stem = name.rsplit_once('.').map(|(s, _)| s.to_string()).unwrap_or(name.clone());
            hh_file = Some(stem);
        }
        if (name.contains("dvper") || name.contains("dv_per")) && (name.ends_with(".tab") || name.ends_with(".csv")) {
            let stem = name.rsplit_once('.').map(|(s, _)| s.to_string()).unwrap_or(name.clone());
            person_file = Some(stem);
        }
    }

    let hh = hh_file.ok_or_else(|| anyhow::anyhow!(
        "No LCFS household file (dvhh*.tab) found in {:?} for {}/{}",
        data_dir, fiscal_year, (fiscal_year + 1) % 100
    ))?;
    let per = person_file.ok_or_else(|| anyhow::anyhow!(
        "No LCFS person file (dvper*.tab) found in {:?} for {}/{}",
        data_dir, fiscal_year, (fiscal_year + 1) % 100
    ))?;

    Ok((hh, per))
}
