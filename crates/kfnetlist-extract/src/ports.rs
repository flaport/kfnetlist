//! Port comparison and grid conversion over native RLayout values.
use crate::{Port, PortTransform};
use rlayout::{db, Error};

pub use kfnetlist_core::port_check::*;

#[derive(Clone, Copy, Debug)]
pub struct CheckOptions {
    pub tolerance: f64,
    pub angle_tolerance: f64,
    pub snapped: bool,
}
impl Default for CheckOptions {
    fn default() -> Self {
        Self {
            tolerance: 0.1,
            angle_tolerance: 0.01,
            snapped: false,
        }
    }
}
impl Port {
    pub fn grid_transform(&self) -> Result<db::Trans, Error> {
        match self.transform {
            PortTransform::Grid(value) | PortTransform::Both(value, _) => Ok(value),
            PortTransform::Physical(value) => value.to_itype(self.dbu)?.to_orthogonal(),
        }
    }
    pub fn physical_transform(&self) -> Result<db::DCplxTrans, Error> {
        match self.transform {
            PortTransform::Physical(value) | PortTransform::Both(_, value) => Ok(value),
            PortTransform::Grid(value) => Ok(value.to_dtype(self.dbu)?.into()),
        }
    }
    /// Own the metadata because each array member has independent geometry.
    pub fn transformed(&self, transform: db::Trans) -> Result<Self, Error> {
        let mut port = self.clone();
        port.transform = match self.transform {
            PortTransform::Grid(value) | PortTransform::Both(value, _) => {
                PortTransform::Grid(value.then(transform)?)
            }
            PortTransform::Physical(value) => {
                PortTransform::Physical(value.then(transform.to_dtype(self.dbu)?.into())?)
            }
        };
        Ok(port)
    }
}
fn quarter_turns(rotation: db::Rotation) -> i32 {
    match rotation {
        db::Rotation::R0 => 0,
        db::Rotation::R90 => 1,
        db::Rotation::R180 => 2,
        db::Rotation::R270 => 3,
    }
}
fn has_grid(transform: PortTransform) -> bool {
    matches!(
        transform,
        PortTransform::Grid(_) | PortTransform::Both(_, _)
    )
}
/// Compare geometry and metadata. Cross-section keys describe equality supplied
/// by the caller, without putting language objects or callbacks in this API.
pub fn check_connection(a: &Port, b: &Port, options: CheckOptions) -> Result<u8, Error> {
    let mut flags = 0;
    if options.snapped || (has_grid(a.transform) && has_grid(b.transform)) {
        let (a, b) = (a.grid_transform()?, b.grid_transform()?);
        if a.displacement == b.displacement {
            flags |= POSITION;
        }
        match (quarter_turns(a.rotation) - quarter_turns(b.rotation)).rem_euclid(4) {
            2 => flags |= OPPOSITE,
            0 => flags |= SAME,
            _ => {}
        }
    } else {
        let (ta, tb) = (a.physical_transform()?, b.physical_transform()?);
        let (pa, pb) = (ta.displacement()?, tb.displacement()?);
        // Match the reference's sqrt(x*x+y*y), including strict tolerances.
        let dx = pa.x() - pb.x();
        let dy = pa.y() - pb.y();
        if (dx * dx + dy * dy).sqrt() < a.dbu * options.tolerance {
            flags |= POSITION;
        }
        let angle = (ta.angle()? - tb.angle()?).rem_euclid(360.0);
        if (angle - 180.0).abs() < options.angle_tolerance {
            flags |= OPPOSITE;
        } else if angle.abs() < options.angle_tolerance {
            flags |= SAME;
        }
    }
    if a.cross_section == b.cross_section {
        flags |= CROSS_SECTION | WIDTH | LAYER;
    } else {
        if a.layer.logical_eq(&b.layer) {
            flags |= LAYER;
        }
        if a.width == b.width {
            flags |= WIDTH;
        }
    }
    if a.port_type == b.port_type {
        flags |= PORT_TYPE;
    }
    Ok(flags)
}
