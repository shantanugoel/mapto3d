/// Road classification based on OSM highway tags
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoadClass {
    Motorway,
    Primary,
    Secondary,
    Tertiary,
    Residential,
}

impl RoadClass {
    /// Classify a highway tag value into a RoadClass
    pub fn from_highway_tag(tag: &str) -> Option<RoadClass> {
        match tag {
            "motorway" | "motorway_link" => Some(RoadClass::Motorway),
            "trunk" | "trunk_link" | "primary" | "primary_link" => Some(RoadClass::Primary),
            "secondary" | "secondary_link" => Some(RoadClass::Secondary),
            "tertiary" | "tertiary_link" => Some(RoadClass::Tertiary),
            "residential" | "living_street" | "unclassified" | "service" => {
                Some(RoadClass::Residential)
            }
            _ => None, // Skip unknown road types
        }
    }
}

/// A road segment with coordinates and classification
#[derive(Debug, Clone)]
pub struct RoadSegment {
    /// Points as (lat, lon) pairs in WGS84
    pub points: Vec<(f64, f64)>,
    /// Road classification
    pub class: RoadClass,
}

impl RoadSegment {
    pub fn new(points: Vec<(f64, f64)>, class: RoadClass) -> Self {
        Self { points, class }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_road_class_from_tag() {
        assert_eq!(
            RoadClass::from_highway_tag("motorway"),
            Some(RoadClass::Motorway)
        );
        assert_eq!(
            RoadClass::from_highway_tag("primary"),
            Some(RoadClass::Primary)
        );
        assert_eq!(
            RoadClass::from_highway_tag("residential"),
            Some(RoadClass::Residential)
        );
        assert_eq!(RoadClass::from_highway_tag("footway"), None);
    }
}
