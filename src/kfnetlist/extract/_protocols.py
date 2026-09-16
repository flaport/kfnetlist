"""Structural types for the optional kfactory/klayout integration."""

from __future__ import annotations

from typing import TYPE_CHECKING, Protocol

from kfnetlist.port_check import _CrossSectionLike, _KCLLike as _PortKCLLike

if TYPE_CHECKING:
    from collections.abc import Iterable, Mapping, Sequence

    from klayout import db as kdb


class FactoryLike(Protocol):
    lvs_equivalent_ports: list[list[str]] | None


class SettingsLike(Protocol):
    def model_dump(self) -> dict[str, object]: ...


class LibraryLike(Protocol):
    def name(self) -> str: ...


class BaseLike(Protocol):
    trans: kdb.Trans | None
    dcplx_trans: kdb.DCplxTrans | None
    port_type: str
    name: str

    @property
    def kcl(self) -> _PortKCLLike: ...

    def transformed(
        self,
        trans: kdb.Trans | kdb.DCplxTrans,
        post_trans: kdb.Trans | kdb.DCplxTrans = ...,
    ) -> BaseLike: ...


class CrossSectionWrapperLike(Protocol):
    @property
    def base(self) -> _CrossSectionLike: ...


class PortLike(Protocol):
    name: str
    port_type: str
    trans: kdb.Trans

    @property
    def layer_info(self) -> kdb.LayerInfo: ...
    @property
    def base(self) -> BaseLike: ...
    @property
    def cross_section(self) -> CrossSectionWrapperLike: ...


class InstanceLike(Protocol):
    name: str
    na: int
    nb: int
    dcplx_trans: kdb.DCplxTrans
    purpose: str | None

    @property
    def instance(self) -> kdb.Instance: ...
    @property
    def cell(self) -> CellLike: ...
    @property
    def ports(self) -> Iterable[PortLike]: ...
    def is_named(self) -> bool: ...


class CellLike(Protocol):
    name: str

    @property
    def virtual(self) -> bool: ...
    @property
    def lvs_equivalent_ports(self) -> list[list[str]] | None: ...
    @property
    def factory_name(self) -> str: ...
    @property
    def settings(self) -> SettingsLike: ...
    @property
    def library_cell(self) -> CellLike: ...
    @property
    def kcl(self) -> KCLLike: ...
    @property
    def ports(self) -> Iterable[PortLike]: ...
    @property
    def insts(self) -> Iterable[InstanceLike]: ...
    def has_factory_name(self) -> bool: ...
    def is_library_cell(self) -> bool: ...
    def library(self) -> LibraryLike: ...
    def cell_index(self) -> int: ...


class KCLLike(Protocol):
    name: str
    layout: kdb.Layout

    @property
    def dbu(self) -> float: ...
    @property
    def connectivity(self) -> Sequence[Sequence[kdb.LayerInfo]]: ...
    @property
    def factories(self) -> Mapping[str, FactoryLike]: ...
    @property
    def virtual_factories(self) -> Mapping[str, FactoryLike]: ...
    def __getitem__(self, key: int | str, /) -> CellLike: ...


class RootCellLike(CellLike, Protocol):
    def called_cells(self) -> Iterable[int]: ...


class DBoxLike(Protocol):
    left: float
    bottom: float
    right: float
    top: float


class DVectorLike(Protocol):
    x: float
    y: float


class DCplxTransLike(Protocol):
    angle: float
    mirror: bool

    @property
    def disp(self) -> DVectorLike: ...


class InstanceShapeLike(Protocol):
    def dbbox(self) -> DBoxLike: ...


class PlaceableLike(Protocol):
    @property
    def instance(self) -> InstanceShapeLike: ...
    @property
    def dcplx_trans(self) -> DCplxTransLike: ...


class ElectricalPortLike(Protocol):
    name: str
    port_type: str
    trans: kdb.Trans

    @property
    def layer_info(self) -> kdb.LayerInfo: ...


class ElectricalCellLike(Protocol):
    name: str

    @property
    def factory_name(self) -> str: ...
    @property
    def ports(self) -> Iterable[ElectricalPortLike]: ...
    def has_factory_name(self) -> bool: ...
    def cell_index(self) -> int: ...


class ElectricalKCLLike(Protocol):
    layout: kdb.Layout

    @property
    def connectivity(self) -> Sequence[Sequence[kdb.LayerInfo]]: ...
    def __getitem__(self, key: int, /) -> ElectricalCellLike: ...


class ElectricalRootCellLike(ElectricalCellLike, Protocol):
    @property
    def kcl(self) -> ElectricalKCLLike: ...
    def called_cells(self) -> Iterable[int]: ...
