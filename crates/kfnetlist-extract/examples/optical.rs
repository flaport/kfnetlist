//! Complete optical extraction through the same domain entry point as Python.
use kfnetlist_extract::{
    db::*,
    electrical::MarkerCell,
    extract::{self, CellInput, Options},
    Port, PortTransform,
};
fn main() -> Result<(), std::boxed::Box<dyn std::error::Error>> {
    let mut layout = Layout::new()?;
    layout.set_dbu(0.001)?;
    let root = layout.create_cell("TOP")?;
    let port = |name: &str, rotation| Port {
        name: name.into(),
        port_type: "optical".into(),
        transform: PortTransform::Grid(Trans::new(rotation, false, Vector::new(0, 0))),
        layer: LayerInfo::new(1, 0),
        width: 500,
        dbu: 0.001,
        cross_section: 0,
    };
    let cells = vec![CellInput {
        id: root,
        name: "TOP".into(),
        ports: vec!["a".into(), "b".into()],
        optical_ports: vec![port("a", Rotation::R0), port("b", Rotation::R180)],
        optical_instances: vec![],
        instances: vec![],
        equivalents: None,
    }];
    let result = extract::extract(
        &layout,
        root,
        &[MarkerCell {
            name: "TOP".into(),
            factory: None,
            ports: vec![],
        }],
        cells,
        Options::default(),
    )?;
    drop(layout);
    let top = &result.netlists["TOP"];
    assert_eq!(top.ports.len(), 2);
    assert_eq!(top.nets.len(), 1);
    assert_eq!(top.nets[0].members.len(), 2);
    println!("{}", kfnetlist_core::to_json(&top.nets)?);
    Ok(())
}
