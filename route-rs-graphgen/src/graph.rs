use std::path::PathBuf;
use xml::EventReader;
use std::io::BufReader;
use std::fs::File;
use std::convert::TryFrom;
use xml::reader::XmlEvent;
use xml::name::OwnedName;
use xml::attribute::OwnedAttribute;
use std::collections::HashMap;

enum GraphNodeKind {
    IO,
    Process,
}

pub(crate) struct Graph {
    pub name: String,
}

impl TryFrom<&PathBuf> for Graph {
    type Error = Vec<String>;

    fn try_from(value: &PathBuf) -> Result<Self, Self::Error> {
        match File::open(value) {
            Ok(file) => {
                let xml_events = EventReader::new(BufReader::new(file));

                let mut name: Option<String> = None;
                let mut nodes: HashMap<String, (GraphNodeKind, String)> = HashMap::new();
                let mut edges: HashMap<String, (String, String)> = HashMap::new();

                for event in xml_events {
                    match event {
                        Ok(XmlEvent::StartElement {
                               name: OwnedName { local_name: xml_node_name, .. },
                               attributes: attrs,
                               ..
                           }) => {
                            match xml_node_name.as_str() {
                                "diagram" => {
                                    name = get_attr(&attrs, "name");
                                },
                                "mxCell" => {
                                    if get_attr(&attrs, "vertex") == Some("1".to_string()) {
                                        let styles = get_styles(&attrs);
                                        if styles.contains_key("ellipse") {
                                            nodes.insert(
                                                get_attr(&attrs, "id").unwrap(),
                                                (
                                                    GraphNodeKind::IO,
                                                    get_attr(&attrs, "value").unwrap(),
                                                )
                                            );
                                        } else {
                                            nodes.insert(
                                                get_attr(&attrs, "id").unwrap(),
                                                (
                                                    GraphNodeKind::Process,
                                                    get_attr(&attrs, "value").unwrap(),
                                                )
                                            );
                                        }
                                    } else if get_attr(&attrs, "edge") == Some("1".to_string()) {
                                        edges.insert(
                                            get_attr(&attrs, "id").unwrap(),
                                            (
                                                get_attr(&attrs, "source").unwrap(),
                                                get_attr(&attrs, "target").unwrap(),
                                            )
                                        );
                                    }
                                }
                                _ => ()
                            }
                        },
                        _ => (),
                    }
                }

                match name {
                    Some(n) => Ok(Graph {
                        name: n,
                    }),
                    _ => {
                        let mut errors = vec![];
                        if name == None { errors.push(String::from("<diagram> must have name attribute")) }
                        Err(errors)
                    }
                }
            },
            Err(e) => Err(vec![format!("{}", e)])
        }
    }
}

/// Helper method to extract an attribute from the attributes vector.
fn find_attr<'a>(attributes: &'a [OwnedAttribute], name: &str) -> Option<&'a OwnedAttribute> {
    attributes.iter().find(|a| a.name.local_name == name)
}

/// Returns true if the attribute is set, or false otherwise.
fn has_attr(attributes: &[OwnedAttribute], name: &str) -> bool {
    find_attr(attributes, name).is_some()
}

/// Returns Some(string_value) if the attribute is set, or None otherwise.
fn get_attr(attributes: &[OwnedAttribute], name: &str) -> Option<String> {
    match find_attr(attributes, name) {
        Some(a) => Some(a.value.clone()),
        None => None,
    }
}

/// From a Vector of xml attributes, construct a HashMap representing the style attribute
/// and its subattributes. If the style attribute is unset, return an empty HashMap.
fn get_styles(attributes: &[OwnedAttribute]) -> HashMap<String, String> {
    let mut style_map = HashMap::new();
    if let Some(style_attr) = get_attr(&attributes, "style") {
        let styles: Vec<&str> = style_attr.split(';').collect();
        for s in styles {
            if s.contains('=') {
                let kv: Vec<&str> = s.splitn(2, '=').collect();
                style_map.insert(String::from(kv[0]), String::from(kv[1]));
            } else {
                style_map.insert(String::from(s), String::from(""));
            }
        }
    }

    style_map
}
