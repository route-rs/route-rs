use petgraph::graph::NodeIndex;
use petgraph::{Directed, Graph};
use std::collections::HashMap;
use std::io::Read;
use xml::attribute::OwnedAttribute;
use xml::name::OwnedName;
use xml::reader::{EventReader, XmlEvent};

pub type XmlNodeId = String;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum NodeKind {
    Element,
    IO,
}

impl Default for NodeKind {
    fn default() -> NodeKind {
        NodeKind::Element
    }
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Hash)]
pub struct NodeData {
    pub xml_node_id: XmlNodeId,
    pub node_class: String,
    pub node_kind: NodeKind,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Hash)]
pub struct EdgeData {
    pub xml_node_id: XmlNodeId,
    pub source: XmlNodeId,
    pub target: XmlNodeId,
}

pub struct PipelineGraph {
    graph: Graph<NodeData, EdgeData, Directed>,
}

impl PipelineGraph {
    pub fn new<R: Read>(xml_source: EventReader<R>) -> Self {
        let (nodes, edges) = nodes_edges_from_xml(xml_source);

        let mut graph = Graph::<NodeData, EdgeData, Directed>::new();

        let mut node_map = HashMap::<XmlNodeId, NodeIndex>::new();

        for n in nodes {
            let node_name = n.xml_node_id.clone();
            let index = graph.add_node(n);
            node_map.insert(node_name, index);
        }

        for e in edges {
            let source_index = node_map[&e.source];
            let target_index = node_map[&e.target];
            graph.extend_with_edges(&[(source_index, target_index, e)]);
        }

        PipelineGraph { graph }
    }

    /// Provides a vector of all nodes in the graph, in arbitrary order.
    #[allow(dead_code)] // This method isn't used but it feels ridiculous to not implement it
    pub fn nodes(&self) -> Vec<&NodeData> {
        self.graph
            .node_indices()
            .map(|i| self.graph.node_weight(i).unwrap())
            .collect()
    }

    /// Provides a vector of all edges in the graph, in arbitrary order.
    pub fn edges(&self) -> Vec<&EdgeData> {
        self.graph
            .edge_indices()
            .map(|i| self.graph.edge_weight(i).unwrap())
            .collect()
    }

    /// Provides a vector of all nodes in the graph sorted topologically.
    pub fn ordered_nodes(&self) -> Vec<&NodeData> {
        let mut nodes = vec![];
        let mut topo = petgraph::visit::Topo::new(&self.graph);
        while let Some(node_index) = topo.next(&self.graph) {
            nodes.push(&self.graph[node_index]);
        }

        nodes
    }
}

#[cfg(test)]
#[allow(non_snake_case)]
mod PipelineGraph_ordered_nodes {
    use super::*;
    use std::collections::HashSet;
    use std::io::Cursor;
    use std::iter::FromIterator;

    #[test]
    fn contains_all_nodes() {
        let xml = r#"
            <?xml version="1.0" encoding=\"UTF-8\"?>
            <mxGraphModel>
                <root>
                    <mxCell id="fooasdfbar-1" style="rhombus" vertex="1" value="FooAsdfBar">
                        <mxGeometry width="100" height="100" as="geometry">
                    </mxCell>
                    <mxCell id="fooasdfbar-2" style="rhombus" vertex="1" value="FooAsdfBar">
                        <mxGeometry width="100" height="100" as="geometry">
                    </mxCell>
                    <mxCell id="fooasdfbar-3" style="rhombus" vertex="1" value="FooAsdfBar">
                        <mxGeometry width="100" height="100" as="geometry">
                    </mxCell>
                    <mxCell id="fooasdfbar-4" style="rhombus" vertex="1" value="FooAsdfBar">
                        <mxGeometry width="100" height="100" as="geometry">
                    </mxCell>
                    <mxCell id="fooasdfbar-5" edge="1" source="fooasdfbar-1" target="fooasdfbar-3"/>
                    <mxCell id="fooasdfbar-6" edge="1" source="fooasdfbar-2" target="fooasdfbar-3"/>
                    <mxCell id="fooasdfbar-7" edge="1" source="fooasdfbar-3" target="fooasdfbar-4"/>
                </root>
            </mxGraphModel>
        "#;

        let pg = PipelineGraph::new(EventReader::new(Cursor::new(xml)));
        let nodes = pg.nodes();
        let nodes_set: HashSet<&&NodeData> = HashSet::from_iter(nodes.iter());
        let ordered_nodes = pg.ordered_nodes();
        let ordered_nodes_set: HashSet<&&NodeData> = HashSet::from_iter(ordered_nodes.iter());

        assert_eq!(nodes_set, ordered_nodes_set);
    }
}

/// Given an EventReader of XML source code, returns a vector of nodes and a vector of edges
/// extracted from that source.
///
/// Nodes with the rhombus shape are considered IO types. Nodes with the default shape are
/// considered Element types.
fn nodes_edges_from_xml<R: Read>(xml_source: EventReader<R>) -> (Vec<NodeData>, Vec<EdgeData>) {
    let mut nodes = vec![];
    let mut edges = vec![];

    for event in xml_source {
        if let Ok(XmlEvent::StartElement {
            name:
                OwnedName {
                    local_name: xml_node_name,
                    ..
                },
            attributes: attrs,
            ..
        }) = event
        {
            if xml_node_name == "mxCell" {
                if has_attr(&attrs, "vertex") {
                    let styles = get_styles(&attrs);
                    nodes.push(NodeData {
                        xml_node_id: get_attr(&attrs, "id").unwrap(),
                        node_class: get_attr(&attrs, "value").unwrap(),
                        node_kind: if styles.contains_key("rhombus") {
                            NodeKind::IO
                        } else {
                            NodeKind::Element
                        },
                    });
                } else if has_attr(&attrs, "edge") {
                    edges.push(EdgeData {
                        xml_node_id: get_attr(&attrs, "id").unwrap(),
                        source: get_attr(&attrs, "source").unwrap(),
                        target: get_attr(&attrs, "target").unwrap(),
                    });
                }
                // Ignore other xml node types
            }
        }
    }

    (nodes, edges)
}

#[cfg(test)]
mod nodes_edges_from_xml {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn rhombus_xml() {
        let xml = r#"
            <?xml version="1.0" encoding="UTF-8"?>
            <mxGraphModel>
                <root>
                    <mxCell id="fooasdfbar-1" style="rhombus" vertex="1" value="FooAsdfBar">
                        <mxGeometry width="100" height="100" as="geometry">
                    </mxCell>
                </root>
            </mxGraphModel>
        "#;

        let (nodes, _) = nodes_edges_from_xml(EventReader::new(Cursor::new(xml)));

        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].node_kind, NodeKind::IO);
    }

    #[test]
    fn rect_xml() {
        let xml = r#"
            <?xml version="1.0" encoding="UTF-8"?>
            <mxGraphModel>
                <root>
                    <mxCell id="fooasdfbar-1" style="" vertex="1" value="FooAsdfBar">
                        <mxGeometry width="100" height="100" as="geometry">
                    </mxCell>
                </root>
            </mxGraphModel>
        "#;

        let (nodes, _) = nodes_edges_from_xml(EventReader::new(Cursor::new(xml)));

        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].node_kind, NodeKind::Element);
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

#[cfg(test)]
mod get_styles {
    use super::*;

    fn attributes_from_map(attr_map: HashMap<&str, &str>) -> Vec<OwnedAttribute> {
        attr_map
            .iter()
            .map(|(k, v)| OwnedAttribute {
                name: OwnedName {
                    local_name: String::from(*k),
                    namespace: None,
                    prefix: None,
                },
                value: String::from(*v),
            })
            .collect()
    }

    #[test]
    fn empty_styles() {
        let styles = get_styles(&[]);

        assert!(styles.is_empty());
    }

    #[test]
    fn short_styles() {
        let attrs = [("style", "alpha;beta")];
        let styles = get_styles(&attributes_from_map(attrs.iter().cloned().collect()));

        assert_eq!(styles.len(), 2);
        assert_eq!(styles["alpha"], "");
        assert_eq!(styles["beta"], "");
    }

    #[test]
    fn long_styles() {
        let attrs = [("style", "alpha=1;beta=gamma")];
        let styles = get_styles(&attributes_from_map(attrs.iter().cloned().collect()));

        assert_eq!(styles.len(), 2);
        assert_eq!(styles["alpha"], "1");
        assert_eq!(styles["beta"], "gamma");
    }

    #[test]
    fn mixed_styles() {
        let attrs = [("style", "alpha;beta=12345")];
        let styles = get_styles(&attributes_from_map(attrs.iter().cloned().collect()));

        assert_eq!(styles.len(), 2);
        assert_eq!(styles["alpha"], "");
        assert_eq!(styles["beta"], "12345");
    }
}
