use serde::de::Error as _;
use serde::ser::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::HashSet;
use uuid::Uuid;

pub const DRAW_SCHEMA_VERSION: u8 = 2;

#[derive(Debug, Clone, PartialEq)]
pub struct DrawData {
    pub version: u8,
    pub width: f64,
    pub height: f64,
    pub background: Option<String>,
    pub elements: Vec<DrawItem>,
}

impl Default for DrawData {
    fn default() -> Self {
        Self {
            version: DRAW_SCHEMA_VERSION,
            width: 1000.0,
            height: 1000.0,
            background: None,
            elements: Vec::new(),
        }
    }
}

impl DrawData {
    pub fn validate(&self) -> Result<(), String> {
        if self.version != DRAW_SCHEMA_VERSION {
            return Err(format!(
                "draw schema version {} cannot be serialized as version {DRAW_SCHEMA_VERSION}",
                self.version
            ));
        }
        if !self.width.is_finite() || !self.height.is_finite() {
            return Err("draw dimensions must be finite".to_string());
        }

        let mut ids = HashSet::with_capacity(self.elements.len());
        for item in &self.elements {
            item.id.validate()?;
            if !ids.insert(item.id.as_str()) {
                return Err(format!("draw item ID '{}' is duplicated", item.id));
            }
            item.element.validate()?;
            item.transform.validate_for(&item.element)?;
        }
        Ok(())
    }

    /// Append an item without changing its identity or the existing item order.
    pub fn add_item(&mut self, item: DrawItem) -> Result<(), String> {
        self.edit(|data| {
            data.elements.push(item);
            Ok(())
        })
    }

    /// Replace an item in place; the replacement must retain the target ID.
    pub fn update_item(&mut self, id: &DrawItemId, item: DrawItem) -> Result<(), String> {
        if item.id != *id {
            return Err("replacement draw item must retain the target ID".to_string());
        }
        self.edit(|data| {
            let target = data
                .item_mut(id)
                .ok_or_else(|| format!("draw item '{id}' does not exist"))?;
            *target = item;
            Ok(())
        })
    }

    pub fn delete_item(&mut self, id: &DrawItemId) -> Result<(), String> {
        self.edit(|data| {
            let index = data
                .elements
                .iter()
                .position(|item| item.id == *id)
                .ok_or_else(|| format!("draw item '{id}' does not exist"))?;
            data.elements.remove(index);
            Ok(())
        })
    }

    /// Replace the item's absolute transform, using the native element rules.
    pub fn transform_item(
        &mut self,
        id: &DrawItemId,
        transform: DrawTransform,
    ) -> Result<(), String> {
        let mut item = self
            .item(id)
            .ok_or_else(|| format!("draw item '{id}' does not exist"))?
            .clone();
        item.transform = transform;
        self.update_item(id, item)
    }

    fn edit(&mut self, change: impl FnOnce(&mut Self) -> Result<(), String>) -> Result<(), String> {
        let mut next = self.clone();
        change(&mut next)?;
        next.validate()?;
        *self = next;
        Ok(())
    }

    #[must_use]
    pub fn item(&self, id: &DrawItemId) -> Option<&DrawItem> {
        self.elements.iter().find(|item| &item.id == id)
    }

    pub fn item_mut(&mut self, id: &DrawItemId) -> Option<&mut DrawItem> {
        self.elements.iter_mut().find(|item| &item.id == id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DrawItemId(pub String);

impl DrawItemId {
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    fn validate(&self) -> Result<(), String> {
        if self.0.is_empty() {
            return Err("draw item ID cannot be empty".to_string());
        }
        let uuid = Uuid::parse_str(&self.0)
            .map_err(|_| format!("draw item ID '{}' is not a UUID", self.0))?;
        if uuid.get_version_num() != 4 {
            return Err(format!("draw item ID '{}' is not a UUID v4", self.0));
        }
        Ok(())
    }
}

impl Default for DrawItemId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for DrawItemId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(formatter)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DrawItem {
    pub id: DrawItemId,
    pub element: DrawElement,
    pub transform: DrawTransform,
}

impl DrawItem {
    #[must_use]
    pub fn new(element: DrawElement) -> Self {
        let (pivot_x, pivot_y) =
            crate::geometry::base_bounds(&element).map_or((0.0, 0.0), |bounds| bounds.center());
        Self {
            id: DrawItemId::new(),
            transform: DrawTransform::identity(pivot_x, pivot_y),
            element,
        }
    }
}

#[must_use]
pub fn pasted_item(item: &DrawItem) -> DrawItem {
    DrawItem {
        id: DrawItemId::new(),
        element: item.element.clone(),
        transform: item.transform,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DrawTransform {
    pub pivot_x: f64,
    pub pivot_y: f64,
    pub translate_x: f64,
    pub translate_y: f64,
    pub rotation_degrees: f64,
    pub scale: f64,
}

impl DrawTransform {
    #[must_use]
    pub const fn identity(pivot_x: f64, pivot_y: f64) -> Self {
        Self {
            pivot_x,
            pivot_y,
            translate_x: 0.0,
            translate_y: 0.0,
            rotation_degrees: 0.0,
            scale: 1.0,
        }
    }

    fn validate_for(&self, element: &DrawElement) -> Result<(), String> {
        let fields = [
            ("pivot_x", self.pivot_x),
            ("pivot_y", self.pivot_y),
            ("translate_x", self.translate_x),
            ("translate_y", self.translate_y),
            ("rotation_degrees", self.rotation_degrees),
            ("scale", self.scale),
        ];
        for (name, value) in fields {
            if !value.is_finite() {
                return Err(format!("draw transform {name} must be finite"));
            }
        }
        if self.scale <= 0.0 {
            return Err("draw transform scale must be positive".to_string());
        }
        if matches!(element, DrawElement::Text(_))
            && (self.rotation_degrees != 0.0 || self.scale != 1.0)
        {
            return Err("text draw items require identity rotation and scale".to_string());
        }
        Ok(())
    }
}

impl Default for DrawTransform {
    fn default() -> Self {
        Self::identity(0.0, 0.0)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct V2DrawData {
    version: u8,
    #[serde(default = "default_draw_dimension")]
    width: f64,
    #[serde(default = "default_draw_dimension")]
    height: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    background: Option<String>,
    elements: Vec<DrawItem>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyDrawData {
    #[serde(default)]
    version: Option<u8>,
    #[serde(default)]
    width: Option<f64>,
    #[serde(default)]
    height: Option<f64>,
    #[serde(default)]
    background: Option<String>,
    #[serde(default)]
    elements: Vec<LegacyDrawElement>,
}

#[derive(Deserialize)]
enum LegacyDrawElement {
    Stroke(Stroke),
    Shape(Shape),
    Text(Text),
    Image(serde::de::IgnoredAny),
}

#[derive(Deserialize)]
#[serde(untagged)]
enum DrawDataWire {
    V2(V2DrawData),
    Legacy(LegacyDrawData),
}

impl Serialize for DrawData {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.validate().map_err(S::Error::custom)?;
        V2DrawData {
            version: DRAW_SCHEMA_VERSION,
            width: self.width,
            height: self.height,
            background: self.background.clone(),
            elements: self.elements.clone(),
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for DrawData {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = DrawDataWire::deserialize(deserializer).map_err(|error| {
            D::Error::custom(format!(
                "unsupported or malformed draw document; check schema, fields and element records: {error}"
            ))
        })?;
        match wire {
            DrawDataWire::V2(data) => {
                if data.version > DRAW_SCHEMA_VERSION {
                    return Err(D::Error::custom(format!(
                        "unsupported draw schema version {}; newest supported version is {DRAW_SCHEMA_VERSION}",
                        data.version
                    )));
                }
                // Empty legacy documents also match the v2 wire shape.
                if data.version != DRAW_SCHEMA_VERSION && !data.elements.is_empty() {
                    return Err(D::Error::custom(format!(
                        "draw schema version {} uses v2 item records",
                        data.version
                    )));
                }
                let data = Self {
                    version: DRAW_SCHEMA_VERSION,
                    width: data.width,
                    height: data.height,
                    background: data.background,
                    elements: data.elements,
                };
                data.validate().map_err(D::Error::custom)?;
                Ok(data)
            }
            DrawDataWire::Legacy(data) => {
                let legacy_version = data.version.unwrap_or(0);
                if legacy_version > DRAW_SCHEMA_VERSION {
                    return Err(D::Error::custom(format!(
                        "unsupported draw schema version {legacy_version}; newest supported version is {DRAW_SCHEMA_VERSION}"
                    )));
                }
                if legacy_version == DRAW_SCHEMA_VERSION {
                    return Err(D::Error::custom(
                        "draw schema version 2 requires item IDs and transforms",
                    ));
                }

                let elements = data
                    .elements
                    .into_iter()
                    .map(|element| match element {
                        LegacyDrawElement::Stroke(stroke) => {
                            Ok(DrawItem::new(DrawElement::Stroke(stroke)))
                        }
                        LegacyDrawElement::Shape(shape) => {
                            Ok(DrawItem::new(DrawElement::Shape(shape)))
                        }
                        LegacyDrawElement::Text(text) => Ok(DrawItem::new(DrawElement::Text(text))),
                        LegacyDrawElement::Image(_) => Err(D::Error::custom(
                            "unsupported legacy Image element; drawing cannot be safely rewritten",
                        )),
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let data = Self {
                    version: DRAW_SCHEMA_VERSION,
                    width: data.width.unwrap_or_else(default_draw_dimension),
                    height: data.height.unwrap_or_else(default_draw_dimension),
                    background: data.background,
                    elements,
                };
                data.validate().map_err(D::Error::custom)?;
                Ok(data)
            }
        }
    }
}

const fn default_draw_dimension() -> f64 {
    1000.0
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DrawElement {
    Stroke(Stroke),
    Shape(Shape),
    Text(Text),
}

impl DrawElement {
    fn validate(&self) -> Result<(), String> {
        let finite = match self {
            Self::Stroke(stroke) => stroke
                .points
                .iter()
                .all(|(x, y)| x.is_finite() && y.is_finite()),
            Self::Shape(
                Shape::Rect {
                    x,
                    y,
                    width,
                    height,
                    ..
                }
                | Shape::Ellipse {
                    x,
                    y,
                    width,
                    height,
                    ..
                }
                | Shape::Diamond {
                    x,
                    y,
                    width,
                    height,
                    ..
                },
            ) => [*x, *y, *width, *height].into_iter().all(f64::is_finite),
            Self::Shape(
                Shape::Line { x1, y1, x2, y2, .. } | Shape::Arrow { x1, y1, x2, y2, .. },
            ) => [*x1, *y1, *x2, *y2].into_iter().all(f64::is_finite),
            Self::Text(text) => text.x.is_finite() && text.y.is_finite(),
        };
        if finite {
            Ok(())
        } else {
            Err("draw element coordinates must be finite".to_string())
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Stroke {
    pub points: Vec<(f64, f64)>,
    pub color: (u8, u8, u8),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum Shape {
    Rect {
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        color: (u8, u8, u8),
    },
    Ellipse {
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        color: (u8, u8, u8),
    },
    Diamond {
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        color: (u8, u8, u8),
    },
    Line {
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
        color: (u8, u8, u8),
    },
    Arrow {
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
        color: (u8, u8, u8),
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DrawShapeType {
    Rect,
    Ellipse,
    Diamond,
    Line,
    Arrow,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Text {
    pub content: String,
    pub x: f64,
    pub y: f64,
    pub color: (u8, u8, u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DrawTool {
    Cursor,
    Draw,
    Erase,
    Text,
    Shape,
}

#[derive(Debug, Clone)]
pub struct Viewport {
    pub x: f64,
    pub y: f64,
    pub zoom: f64,
}

impl Default for Viewport {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            zoom: 1.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stroke() -> DrawElement {
        DrawElement::Stroke(Stroke {
            points: vec![(10.0, 20.0), (30.0, 40.0)],
            color: (1, 2, 3),
        })
    }

    fn v2_item_value(id: &str) -> serde_json::Value {
        serde_json::json!({
            "id": id,
            "element": {"Stroke": {"points": [[0.0, 0.0], [10.0, 10.0]], "color": [1, 2, 3]}},
            "transform": {
                "pivot_x": 5.0,
                "pivot_y": 5.0,
                "translate_x": 1.0,
                "translate_y": 2.0,
                "rotation_degrees": 30.0,
                "scale": 2.0
            }
        })
    }

    #[test]
    fn v2_round_trip_preserves_items_and_transforms() {
        let mut item = DrawItem::new(stroke());
        item.transform = DrawTransform {
            pivot_x: 20.0,
            pivot_y: 30.0,
            translate_x: 5.0,
            translate_y: -8.0,
            rotation_degrees: 90.0,
            scale: 1.5,
        };
        let data = DrawData {
            version: DRAW_SCHEMA_VERSION,
            width: 200.0,
            height: 300.0,
            background: Some("#010203".to_string()),
            elements: vec![item],
        };

        let encoded = serde_json::to_string(&data).unwrap();
        assert!(encoded.contains("\"version\":2"));
        assert_eq!(serde_json::from_str::<DrawData>(&encoded).unwrap(), data);
    }

    #[test]
    fn clean_legacy_versions_migrate_strokes_shapes_and_text() {
        for version in [None, Some(0), Some(1)] {
            let mut source = serde_json::json!({
                "width": 200.0,
                "height": 300.0,
                "elements": [
                    {"Stroke": {"points": [[1.0, 2.0], [3.0, 4.0]], "color": [1, 2, 3]}},
                    {"Shape": {"Rect": {"x": 1.0, "y": 2.0, "width": 3.0, "height": 4.0, "color": [1, 2, 3]}}},
                    {"Text": {"content": "legacy", "x": 20.0, "y": 30.0, "color": [4, 5, 6]}}
                ]
            });
            if let Some(version) = version {
                source["version"] = serde_json::json!(version);
            }

            let data: DrawData = serde_json::from_value(source).unwrap();
            assert_eq!(data.version, DRAW_SCHEMA_VERSION);
            assert_eq!(data.elements.len(), 3);
            assert!(data.elements.iter().all(|item| {
                Uuid::parse_str(item.id.as_str()).is_ok()
                    && item.transform.scale == 1.0
                    && item.transform.rotation_degrees == 0.0
                    && item.transform.translate_x == 0.0
                    && item.transform.translate_y == 0.0
            }));
        }
    }

    #[test]
    fn unknown_fields_are_rejected_at_every_document_layer() {
        let item = v2_item_value(&DrawItemId::new().0);
        let document = serde_json::json!({"version": 2, "elements": [item]});
        for pointer in [
            "",
            "/elements/0",
            "/elements/0/transform",
            "/elements/0/element/Stroke",
        ] {
            let mut source = document.clone();
            source.pointer_mut(pointer).unwrap()["extension"] = serde_json::json!(42);
            let error = serde_json::from_value::<DrawData>(source).unwrap_err();
            assert!(
                error
                    .to_string()
                    .contains("unsupported or malformed draw document"),
                "{error}"
            );
        }
        let elements = [
            DrawElement::Text(Text {
                content: "text".into(),
                x: 1.0,
                y: 2.0,
                color: (1, 2, 3),
            }),
            DrawElement::Shape(Shape::Rect {
                x: 1.0,
                y: 2.0,
                width: 3.0,
                height: 4.0,
                color: (1, 2, 3),
            }),
            DrawElement::Shape(Shape::Ellipse {
                x: 1.0,
                y: 2.0,
                width: 3.0,
                height: 4.0,
                color: (1, 2, 3),
            }),
            DrawElement::Shape(Shape::Diamond {
                x: 1.0,
                y: 2.0,
                width: 3.0,
                height: 4.0,
                color: (1, 2, 3),
            }),
            DrawElement::Shape(Shape::Line {
                x1: 1.0,
                y1: 2.0,
                x2: 3.0,
                y2: 4.0,
                color: (1, 2, 3),
            }),
            DrawElement::Shape(Shape::Arrow {
                x1: 1.0,
                y1: 2.0,
                x2: 3.0,
                y2: 4.0,
                color: (1, 2, 3),
            }),
        ];
        for element in elements {
            let mut value = serde_json::to_value(&element).unwrap();
            let fields = match &element {
                DrawElement::Text(_) => &mut value["Text"],
                DrawElement::Shape(_) => value["Shape"]
                    .as_object_mut()
                    .unwrap()
                    .values_mut()
                    .next()
                    .unwrap(),
                DrawElement::Stroke(_) => unreachable!(),
            };
            fields["extension"] = serde_json::json!(42);
            for version in [0, 1, 2] {
                let record = if version == 2 {
                    let mut record = item.clone();
                    record["element"] = value.clone();
                    record["transform"] = serde_json::to_value(DrawTransform::default()).unwrap();
                    record
                } else {
                    value.clone()
                };
                let source = serde_json::json!({"version": version, "elements": [record]});
                let error = serde_json::from_value::<DrawData>(source).unwrap_err();
                assert!(
                    error
                        .to_string()
                        .contains("unsupported or malformed draw document"),
                    "{error}"
                );
            }
        }
        for version in [None, Some(0), Some(1), Some(2)] {
            let mut source = serde_json::json!({"elements": [], "extension": 42});
            if let Some(version) = version {
                source["version"] = serde_json::json!(version);
            }
            assert!(serde_json::from_value::<DrawData>(source).is_err());
        }
        let mut source = document;
        source["elements"][0]["element"]["extension"] = serde_json::json!(42);
        assert!(serde_json::from_value::<DrawData>(source).is_err());
    }

    #[test]
    fn duplicate_document_and_nested_fields_are_rejected_without_collapsing_data() {
        let data = DrawData {
            elements: vec![DrawItem::new(stroke())],
            ..DrawData::default()
        };
        let source = serde_json::to_string(&data).unwrap();
        for field in [
            "\"version\":2".to_string(),
            format!("\"id\":\"{}\"", data.elements[0].id),
            "\"scale\":1.0".to_string(),
            "\"color\":[1,2,3]".to_string(),
        ] {
            assert!(source.contains(&field));
            let duplicate = source.replacen(&field, &format!("{field},{field}"), 1);
            assert!(
                serde_json::from_str::<DrawData>(&duplicate).is_err(),
                "{duplicate}"
            );
        }
    }

    #[test]
    fn legacy_images_and_mismatched_record_versions_are_rejected() {
        for version in [None, Some(0), Some(1)] {
            let mut source = serde_json::json!({"elements": [{"Image": {"path": "keep.png"}}]});
            if let Some(version) = version {
                source["version"] = serde_json::json!(version);
            }
            assert!(
                serde_json::from_value::<DrawData>(source)
                    .unwrap_err()
                    .to_string()
                    .contains("unsupported legacy Image")
            );
        }
        for source in [
            serde_json::json!({"version": 1, "elements": [v2_item_value(&DrawItemId::new().0)]}),
            serde_json::json!({"version": 2, "elements": [{"Stroke": {"points": [], "color": [1, 2, 3]}}]}),
            serde_json::json!({"version": 3, "extension": 42, "elements": []}),
            serde_json::json!({"version": 256, "elements": []}),
        ] {
            assert!(serde_json::from_value::<DrawData>(source).is_err());
        }
        for version in [0, 1, 2] {
            let source = serde_json::json!({"version": version, "elements": []});
            assert_eq!(
                serde_json::from_value::<DrawData>(source).unwrap(),
                DrawData::default()
            );
        }
    }

    #[test]
    fn semantic_edits_preserve_ids_order_and_round_trip() {
        let mut data = DrawData::default();
        let first = DrawItem::new(stroke());
        let second = DrawItem::new(stroke());
        data.add_item(first.clone()).unwrap();
        data.add_item(second.clone()).unwrap();
        let mut replacement = first.clone();
        replacement.element = DrawElement::Shape(Shape::Line {
            x1: 1.0,
            y1: 2.0,
            x2: 3.0,
            y2: 4.0,
            color: (5, 6, 7),
        });
        data.update_item(&first.id, replacement.clone()).unwrap();
        let transform = DrawTransform {
            translate_x: 3.0,
            translate_y: -4.0,
            rotation_degrees: 180.0,
            scale: 2.0,
            ..replacement.transform
        };
        data.transform_item(&first.id, transform).unwrap();
        assert_eq!(data.elements[0].id, first.id);
        assert_eq!(data.elements[1], second);
        assert_eq!(data.item(&first.id).unwrap().element, replacement.element);
        assert_eq!(data.item(&first.id).unwrap().transform, transform);
        let encoded = serde_json::to_string(&data).unwrap();
        assert_eq!(serde_json::from_str::<DrawData>(&encoded).unwrap(), data);
        data.delete_item(&first.id).unwrap();
        assert_eq!(data.elements, [second]);
    }

    #[test]
    fn invalid_semantic_edits_leave_the_model_unchanged() {
        let first = DrawItem::new(stroke());
        let mut data = DrawData::default();
        data.add_item(first.clone()).unwrap();
        let original = data.clone();
        let missing = DrawItemId::new();
        assert!(
            data.add_item(first.clone())
                .unwrap_err()
                .contains("duplicated")
        );
        assert_eq!(data, original);
        assert!(
            data.delete_item(&missing)
                .unwrap_err()
                .contains("does not exist")
        );
        assert_eq!(data, original);
        assert!(
            data.transform_item(&missing, DrawTransform::default())
                .is_err()
        );
        assert_eq!(data, original);
        assert!(
            data.update_item(
                &missing,
                DrawItem {
                    id: missing.clone(),
                    ..first.clone()
                }
            )
            .is_err()
        );
        assert_eq!(data, original);
        assert!(
            data.update_item(&first.id, DrawItem::new(stroke()))
                .unwrap_err()
                .contains("retain the target ID")
        );
        assert_eq!(data, original);
        for id in ["", "invalid", "00000000-0000-0000-0000-000000000000"] {
            assert!(
                data.add_item(DrawItem {
                    id: DrawItemId(id.into()),
                    ..first.clone()
                })
                .is_err()
            );
            assert_eq!(data, original);
        }
        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let mut item = first.clone();
            item.element = DrawElement::Text(Text {
                content: "invalid".into(),
                x: value,
                y: 0.0,
                color: (1, 2, 3),
            });
            assert!(data.update_item(&first.id, item).is_err());
            assert_eq!(data, original);
            assert!(
                data.add_item(DrawItem::new(DrawElement::Stroke(Stroke {
                    points: vec![(value, 0.0)],
                    color: (1, 2, 3)
                })))
                .is_err()
            );
            assert_eq!(data, original);
        }
        for transform in [
            DrawTransform {
                scale: 0.0,
                ..first.transform
            },
            DrawTransform {
                scale: -1.0,
                ..first.transform
            },
            DrawTransform {
                pivot_x: f64::NAN,
                ..first.transform
            },
            DrawTransform {
                pivot_y: f64::INFINITY,
                ..first.transform
            },
            DrawTransform {
                translate_x: f64::NAN,
                ..first.transform
            },
            DrawTransform {
                translate_y: f64::INFINITY,
                ..first.transform
            },
            DrawTransform {
                rotation_degrees: f64::NAN,
                ..first.transform
            },
            DrawTransform {
                scale: f64::INFINITY,
                ..first.transform
            },
        ] {
            assert!(data.transform_item(&first.id, transform).is_err());
            assert_eq!(data, original);
        }
    }

    #[test]
    fn semantic_text_transforms_allow_translation_but_reject_rotation_and_scale() {
        let mut data = DrawData::default();
        let text = DrawItem::new(DrawElement::Text(Text {
            content: "text".into(),
            x: 1.0,
            y: 2.0,
            color: (1, 2, 3),
        }));
        data.add_item(text.clone()).unwrap();
        let translated = DrawTransform {
            translate_x: 10.0,
            translate_y: -20.0,
            ..text.transform
        };
        data.transform_item(&text.id, translated).unwrap();
        let original = data.clone();
        for transform in [
            DrawTransform {
                rotation_degrees: 1.0,
                ..translated
            },
            DrawTransform {
                scale: 2.0,
                ..translated
            },
        ] {
            assert!(
                data.transform_item(&text.id, transform)
                    .unwrap_err()
                    .contains("identity rotation and scale")
            );
            assert_eq!(data, original);
        }
    }

    #[test]
    fn v2_rejects_future_duplicate_and_invalid_transforms() {
        let id = Uuid::new_v4().to_string();
        let future = serde_json::json!({
            "version": 3,
            "width": 100.0,
            "height": 100.0,
            "elements": [v2_item_value(&id)]
        });
        assert!(
            serde_json::from_value::<DrawData>(future)
                .unwrap_err()
                .to_string()
                .contains("unsupported draw schema version 3")
        );

        let duplicate = serde_json::json!({
            "version": 2,
            "width": 100.0,
            "height": 100.0,
            "elements": [v2_item_value(&id), v2_item_value(&id)]
        });
        assert!(
            serde_json::from_value::<DrawData>(duplicate)
                .unwrap_err()
                .to_string()
                .contains("duplicated")
        );

        let mut data = DrawData::default();
        data.elements.push(DrawItem {
            id: DrawItemId::new(),
            element: DrawElement::Text(Text {
                content: "text".to_string(),
                x: 0.0,
                y: 0.0,
                color: (255, 255, 255),
            }),
            transform: DrawTransform {
                scale: 2.0,
                ..DrawTransform::default()
            },
        });
        assert!(
            data.validate()
                .unwrap_err()
                .contains("identity rotation and scale")
        );

        data.elements[0].element = stroke();
        data.elements[0].transform.scale = 0.0;
        assert!(
            data.validate()
                .unwrap_err()
                .contains("scale must be positive")
        );
    }

    #[test]
    fn validation_rejects_nonfinite_dimensions_and_element_coordinates() {
        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            for (width, height) in [(value, 1.0), (1.0, value)] {
                let data = DrawData {
                    width,
                    height,
                    ..DrawData::default()
                };
                assert!(
                    data.validate()
                        .unwrap_err()
                        .contains("dimensions must be finite")
                );
            }
            for coordinates in [
                [value, 1.0, 2.0, 3.0],
                [1.0, value, 2.0, 3.0],
                [1.0, 2.0, value, 3.0],
                [1.0, 2.0, 3.0, value],
            ] {
                let [x, y, width, height] = coordinates;
                let color = (1, 2, 3);
                let elements = [
                    DrawElement::Stroke(Stroke {
                        points: vec![(0.0, 0.0), (x, y), (width, height)],
                        color,
                    }),
                    DrawElement::Shape(Shape::Rect {
                        x,
                        y,
                        width,
                        height,
                        color,
                    }),
                    DrawElement::Shape(Shape::Ellipse {
                        x,
                        y,
                        width,
                        height,
                        color,
                    }),
                    DrawElement::Shape(Shape::Diamond {
                        x,
                        y,
                        width,
                        height,
                        color,
                    }),
                    DrawElement::Shape(Shape::Line {
                        x1: x,
                        y1: y,
                        x2: width,
                        y2: height,
                        color,
                    }),
                    DrawElement::Shape(Shape::Arrow {
                        x1: x,
                        y1: y,
                        x2: width,
                        y2: height,
                        color,
                    }),
                    DrawElement::Text(Text {
                        content: "text".into(),
                        x: x + width,
                        y: y + height,
                        color,
                    }),
                ];
                for element in elements {
                    let data = DrawData {
                        elements: vec![DrawItem {
                            id: DrawItemId::new(),
                            element,
                            transform: DrawTransform::default(),
                        }],
                        ..DrawData::default()
                    };
                    assert!(
                        data.validate()
                            .unwrap_err()
                            .contains("coordinates must be finite")
                    );
                    assert!(serde_json::to_string(&data).is_err());
                }
            }
        }
    }

    #[test]
    fn clipboard_preserves_item_payload_and_refreshes_id() {
        let mut source = DrawItem::new(stroke());
        source.transform.translate_x = 12.0;
        source.transform.rotation_degrees = 45.0;
        source.transform.scale = 2.0;

        let pasted = pasted_item(&source);
        assert_ne!(pasted.id, source.id);
        assert_eq!(pasted.element, source.element);
        assert_eq!(pasted.transform, source.transform);
    }
}
