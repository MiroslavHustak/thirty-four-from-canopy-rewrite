//    cd c:\temp\
//    chromedriver.exe --port=9515

/// Main URLs for scraping current and future timetables
pub const MAIN_URLS: &[&str] = &[
    "https://www.kodis.cz/lines/city?tab=MHD+Ostrava",
    "https://www.kodis.cz/lines/region?tab=75",
    "https://www.kodis.cz/lines/city?tab=MHD+Opava",
    "https://www.kodis.cz/lines/region?tab=232-293",
    "https://www.kodis.cz/lines/city?tab=MHD+Frýdek-Místek",
    "https://www.kodis.cz/lines/region?tab=331-392",
    "https://www.kodis.cz/lines/city?tab=MHD+Havířov",
    "https://www.kodis.cz/lines/region?tab=440-465",
    "https://www.kodis.cz/lines/city?tab=MHD+Karviná",
    "https://www.kodis.cz/lines/city?tab=MHD+Orlová",
    "https://www.kodis.cz/lines/region?tab=531-583",
    "https://www.kodis.cz/lines/city?tab=MHD+Nový+Jičín",
    "https://www.kodis.cz/lines/city?tab=MHD+Studénka",
    "https://www.kodis.cz/lines/region?tab=613-699",
    "https://www.kodis.cz/lines/city?tab=MHD+Třinec",
    "https://www.kodis.cz/lines/city?tab=MHD+Český+Těšín",
    "https://www.kodis.cz/lines/region?tab=731-788",
    "https://www.kodis.cz/lines/city?tab=MHD+Krnov",
    "https://www.kodis.cz/lines/city?tab=MHD+Bruntál",
    "https://www.kodis.cz/lines/region?tab=811-885",
    "https://www.kodis.cz/lines/region?tab=901-990",
    "https://www.kodis.cz/lines/train?tab=S1-S34",
    "https://www.kodis.cz/lines/train?tab=R8-R62",
    "https://www.kodis.cz/lines/city?tab=NAD+MHD",
    "https://www.kodis.cz/lines/region?tab=NAD",
    "https://www.kodis.cz/lines/boat?tab=Lodní+doprava",
];

/// Base URL for changes pages
pub const CHANGES_BASE_URL: &str = "https://www.kodis.cz/changes/";

/// Generate list of change IDs to scrape
/// Returns: [2115, 2400, 2401, ..., 2799]
pub fn get_change_ids() -> Vec<i32> {
    std::iter::once(2115).chain(2400..2800).collect()
}

/*
/// Helper function to get URLs as Vec<String> instead of &[&str]
/// Some code might need owned Strings instead of string slices
pub fn get_main_urls_owned() -> Vec<String> {
    MAIN_URLS.iter().map(|&s| s.to_string()).collect()
}
*/
