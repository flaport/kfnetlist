//! `cargo run -p kfnetlist-extract --example standalone` needs no Python.
use kfnetlist_extract::connected_geometry;
use rlayout::db::*;

fn main() -> Result<(), std::boxed::Box<dyn std::error::Error>> {
    let mut layout = Layout::new()?;
    let root = layout.create_cell("WIRE")?;
    let metal = layout.layer(&LayerInfo::new(1, 0).with_name("M1"))?;
    layout.cell_mut(root)?.insert_polygon(
        metal,
        &Polygon::from_box(rlayout::db::Box::new(0, 0, 1000, 100))?,
    )?;
    layout.cell_mut(root)?.insert_text(
        metal,
        &Text::new("A", Trans::new(Rotation::R0, false, Vector::new(50, 50)))?,
    )?;
    let extraction = connected_geometry(&layout, root, &[vec![metal]])?;
    // The context and graph retain their real native source after this drop.
    drop(layout);
    let graph = extraction.netlist()?.ok_or("missing extracted graph")?;
    drop(extraction);
    let circuit = graph.circuit_by_name("WIRE")?.ok_or("missing WIRE")?;
    assert_eq!(circuit.nets()?.len(), 1);
    assert_eq!(circuit.nets()?[0].name()?, "A");
    println!("{}", graph.to_native_string()?);
    Ok(())
}
