use std::collections::{HashMap, HashSet};
use std::ops::Deref;

use smallvec::{smallvec, SmallVec};
use smol_str::SmolStr;
use squalid::_d;
use tracing::instrument;

use super::IndexInProduced;
use crate::{
    Carver, CarverList, Database, DatabaseInterface, DependencyType, DependencyValue,
    ExternalDependencyValues, FieldPlan, Id, OptionalPopulator,
    OptionalUnionOrInterfaceTypePopulator, Populator, PopulatorList, UnionOrInterfaceTypePopulator,
    UnionOrInterfaceTypePopulatorList, WheresResolved,
};

#[derive(Clone, Debug)]
pub struct ColumnSpec {
    pub name: SmolStr,
    pub dependency_type: DependencyType,
}

type ColumnSpecs = SmallVec<[ColumnSpec; 12]>;

#[derive(Debug)]
pub enum AsyncStep {
    ListOfColumn {
        table_name: SmolStr,
        column: ColumnSpec,
        wheres: WheresResolved,
    },
    Column(AsyncStepColumn),
    MultipleColumns(AsyncStepMultipleColumns),
    ListOfIdAndFollowOnColumns {
        table_name: SmolStr,
        id_column: ColumnSpec,
        wheres: WheresResolved,
        other_columns: ColumnSpecs,
    },
}

impl AsyncStep {
    #[instrument(level = "trace", skip(self, database))]
    pub async fn run(&self, database: &Database) -> AsyncStepResponse {
        match self {
            Self::ListOfColumn {
                table_name,
                column,
                wheres,
            } => DependencyValue::List(
                database
                    .get_column_list(table_name, &column.name, column.dependency_type, wheres)
                    .await,
            )
            .into(),
            Self::Column(AsyncStepColumn {
                table_name,
                column,
                id_column_name,
                id,
            }) => database
                .get_column(
                    table_name,
                    &column.name,
                    id,
                    id_column_name,
                    column.dependency_type,
                )
                .await
                .into(),
            Self::MultipleColumns(AsyncStepMultipleColumns {
                table_name,
                columns,
                id_column_name,
                id,
            }) => AsyncStepResponse::DependencyValueMap(
                database
                    .as_postgres()
                    .get_columns(table_name, columns, id, id_column_name)
                    .await,
            ),
            Self::ListOfIdAndFollowOnColumns {
                table_name,
                id_column,
                wheres,
                other_columns,
            } => {
                let columns: ColumnSpecs = [id_column]
                    .into_iter()
                    .cloned()
                    .chain(other_columns.into_iter().cloned())
                    .collect();
                AsyncStepResponse::ListOfDependencyValueMap(
                    database
                        .as_postgres()
                        .get_columns_list(table_name, &columns, wheres)
                        .await,
                )
            }
        }
    }

    pub fn as_multiple_columns(&self) -> &AsyncStepMultipleColumns {
        match self {
            Self::MultipleColumns(multiple_columns) => multiple_columns,
            _ => panic!("expected multiple columns"),
        }
    }

    pub fn into_multiple_columns(self) -> AsyncStepMultipleColumns {
        match self {
            Self::MultipleColumns(multiple_columns) => multiple_columns,
            _ => panic!("expected multiple columns"),
        }
    }

    pub fn into_column(self) -> AsyncStepColumn {
        match self {
            Self::Column(column) => column,
            _ => panic!("expected column"),
        }
    }
}

#[derive(Debug)]
pub struct AsyncStepColumn {
    pub table_name: SmolStr,
    pub column: ColumnSpec,
    pub id_column_name: SmolStr,
    pub id: Id,
}

#[derive(Debug)]
pub struct AsyncStepMultipleColumns {
    pub table_name: SmolStr,
    pub columns: ColumnSpecs,
    pub id_column_name: SmolStr,
    pub id: Id,
}

pub enum AsyncStepResponse {
    DependencyValue(DependencyValue),
    DependencyValueMap(HashMap<SmolStr, DependencyValue>),
    ListOfDependencyValueMap(Vec<HashMap<SmolStr, DependencyValue>>),
}

impl AsyncStepResponse {
    pub fn into_dependency_value(self) -> DependencyValue {
        match self {
            Self::DependencyValue(dependency_value) => dependency_value,
            _ => panic!("expected dependency value"),
        }
    }

    pub fn into_dependency_value_map(self) -> HashMap<SmolStr, DependencyValue> {
        match self {
            Self::DependencyValueMap(map) => map,
            _ => panic!("expected dependency value map"),
        }
    }

    pub fn into_list_of_dependency_value_map(self) -> Vec<HashMap<SmolStr, DependencyValue>> {
        match self {
            Self::ListOfDependencyValueMap(map) => map,
            _ => panic!("expected vec of dependency value map"),
        }
    }
}

impl From<DependencyValue> for AsyncStepResponse {
    fn from(value: DependencyValue) -> Self {
        Self::DependencyValue(value)
    }
}

pub type AsyncSteps = SmallVec<[AsyncStep; 4]>;
type IsInternalDependenciesOfs<'a> = SmallVec<[IsInternalDependenciesOf<'a>; 4]>;

pub enum AsyncInstruction<'a> {
    Simple(AsyncInstructionSimple<'a>),
    RowMultipleColumnsEachOfWhichAreOnlyInternalDependency {
        step: AsyncStep,
        is_internal_dependencies_of: HashMap<SmolStr, IsInternalDependenciesOfs<'a>>,
    },
    ListOfIdsAndFollowOnColumnGetters {
        step: AsyncStep,
        list_of_ids_is_internal_dependencies_of: IsInternalDependenciesOf<'a>,
        id_column_name: SmolStr,
        follow_on_columns: HashSet<SmolStr>,
    },
}

impl<'a> AsyncInstruction<'a> {
    pub fn as_simple(&self) -> &AsyncInstructionSimple<'a> {
        match self {
            Self::Simple(simple) => simple,
            _ => panic!("expected simple"),
        }
    }

    pub fn into_simple(self) -> AsyncInstructionSimple<'a> {
        match self {
            Self::Simple(simple) => simple,
            _ => panic!("expected simple"),
        }
    }
}

pub struct AsyncInstructionSimple<'a> {
    pub steps: AsyncSteps,
    pub internal_dependency_names: DependencyNames,
    pub is_internal_dependencies_of: IsInternalDependenciesOf<'a>,
}

type AsyncInstructionsStore<'a> = SmallVec<[AsyncInstruction<'a>; 8]>;

#[derive(Default)]
pub struct AsyncInstructions<'a> {
    pub instructions: AsyncInstructionsStore<'a>,
}

impl<'a> AsyncInstructions<'a> {
    pub fn push(&mut self, instruction: AsyncInstruction<'a>) {
        if let Some(combineable_with_index) =
            is_row_multiple_columns_each_of_which_are_only_internal_dependency_combineable(
                &instruction,
                &self.instructions,
            )
        {
            let AsyncInstructionSimple {
                steps: mut instruction_steps,
                internal_dependency_names: mut instruction_internal_dependency_names,
                is_internal_dependencies_of: instruction_is_internal_dependencies_of,
            } = instruction.into_simple();
            assert_eq!(instruction_steps.len(), 1);
            let instruction_step = instruction_steps.remove(0).into_column();
            let updated_instruction = match self.instructions.remove(combineable_with_index) {
                AsyncInstruction::Simple(AsyncInstructionSimple {
                    mut steps,
                    mut internal_dependency_names,
                    is_internal_dependencies_of,
                }) => {
                    assert_eq!(steps.len(), 1);
                    let step = steps.remove(0).into_column();
                    AsyncInstruction::RowMultipleColumnsEachOfWhichAreOnlyInternalDependency {
                        step: AsyncStep::MultipleColumns(AsyncStepMultipleColumns {
                            table_name: step.table_name,
                            columns: smallvec![step.column, instruction_step.column],
                            id_column_name: step.id_column_name,
                            id: step.id,
                        }),
                        is_internal_dependencies_of: {
                            let mut multi_map: HashMap<SmolStr, IsInternalDependenciesOfs> = _d();
                            multi_map
                                .entry(internal_dependency_names.remove(0))
                                .or_default()
                                .push(is_internal_dependencies_of);
                            multi_map
                                .entry(instruction_internal_dependency_names.remove(0))
                                .or_default()
                                .push(instruction_is_internal_dependencies_of);
                            multi_map
                        },
                    }
                }
                AsyncInstruction::RowMultipleColumnsEachOfWhichAreOnlyInternalDependency {
                    step,
                    mut is_internal_dependencies_of,
                } => {
                    let AsyncStepMultipleColumns {
                        table_name,
                        mut columns,
                        id_column_name,
                        id,
                    } = step.into_multiple_columns();
                    columns.push(instruction_step.column);
                    is_internal_dependencies_of
                        .entry(instruction_internal_dependency_names.remove(0))
                        .or_default()
                        .push(instruction_is_internal_dependencies_of);
                    AsyncInstruction::RowMultipleColumnsEachOfWhichAreOnlyInternalDependency {
                        step: AsyncStep::MultipleColumns(AsyncStepMultipleColumns {
                            table_name,
                            columns,
                            id_column_name,
                            id,
                        }),
                        is_internal_dependencies_of,
                    }
                }
                AsyncInstruction::ListOfIdsAndFollowOnColumnGetters { .. } => unreachable!(),
            };
            self.instructions.push(updated_instruction);
            return;
        }
        self.instructions.push(instruction);
    }
}

impl<'a> Deref for AsyncInstructions<'a> {
    type Target = AsyncInstructionsStore<'a>;

    fn deref(&self) -> &Self::Target {
        &self.instructions
    }
}

impl<'a> IntoIterator for AsyncInstructions<'a> {
    type Item = <AsyncInstructionsStore<'a> as IntoIterator>::Item;
    type IntoIter = <AsyncInstructionsStore<'a> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.instructions.into_iter()
    }
}

fn is_row_multiple_columns_each_of_which_are_only_internal_dependency_combineable(
    instruction: &AsyncInstruction,
    existing: &AsyncInstructionsStore,
) -> Option<usize> {
    let instruction = instruction.as_simple();
    if instruction.steps.len() != 1 {
        return None;
    }
    let AsyncStep::Column(column_step) = &instruction.steps[0] else {
        return None;
    };
    existing
        .into_iter()
        .position(|existing_instruction| match existing_instruction {
            AsyncInstruction::Simple(simple) => {
                if simple.steps.len() != 1 {
                    return false;
                }
                let AsyncStep::Column(existing_column_step) = &simple.steps[0] else {
                    return false;
                };
                existing_column_step.table_name == column_step.table_name
                    && existing_column_step.id_column_name == column_step.id_column_name
                    && existing_column_step.id == column_step.id
            }
            AsyncInstruction::RowMultipleColumnsEachOfWhichAreOnlyInternalDependency {
                step,
                ..
            } => {
                let step = step.as_multiple_columns();
                step.table_name == column_step.table_name
                    && step.id_column_name == column_step.id_column_name
                    && step.id == column_step.id
            }
            _ => false,
        })
}

pub type DependencyNames = SmallVec<[SmolStr; 4]>;

pub enum IsInternalDependenciesOf<'a> {
    ObjectFieldScalar {
        parent_object_index: IndexInProduced,
        index_of_field_in_object: usize,
        carver: &'a Box<dyn Carver>,
        external_dependency_values: ExternalDependencyValues,
        field_name: SmolStr,
    },
    ObjectFieldObject {
        parent_object_index: IndexInProduced,
        index_of_field_in_object: usize,
        populator: &'a Populator,
        external_dependency_values: ExternalDependencyValues,
        field_name: SmolStr,
        field_plan: &'a FieldPlan<'a>,
    },
    ObjectFieldListOfObjects(IsInternalDependenciesOfObjectFieldListOfObjects<'a>),
    ObjectFieldUnionOrInterfaceObject {
        parent_object_index: IndexInProduced,
        index_of_field_in_object: usize,
        type_populator: &'a Box<dyn UnionOrInterfaceTypePopulator>,
        populator: &'a Populator,
        external_dependency_values: ExternalDependencyValues,
        field_name: SmolStr,
        field_plan: &'a FieldPlan<'a>,
    },
    ObjectFieldListOfScalars {
        parent_object_index: IndexInProduced,
        index_of_field_in_object: usize,
        carver: &'a Box<dyn CarverList>,
        external_dependency_values: ExternalDependencyValues,
        field_name: SmolStr,
    },
    ObjectFieldOptionalObject {
        parent_object_index: IndexInProduced,
        index_of_field_in_object: usize,
        populator: &'a OptionalPopulator,
        external_dependency_values: ExternalDependencyValues,
        field_name: SmolStr,
        field_plan: &'a FieldPlan<'a>,
    },
    ObjectFieldOptionalUnionOrInterfaceObject {
        parent_object_index: IndexInProduced,
        index_of_field_in_object: usize,
        type_populator: &'a Box<dyn OptionalUnionOrInterfaceTypePopulator>,
        populator: &'a Populator,
        external_dependency_values: ExternalDependencyValues,
        field_name: SmolStr,
        field_plan: &'a FieldPlan<'a>,
    },
    ObjectFieldListOfUnionOrInterfaceObjects {
        parent_object_index: IndexInProduced,
        index_of_field_in_object: usize,
        type_populator: &'a Box<dyn UnionOrInterfaceTypePopulatorList>,
        populator: &'a PopulatorList,
        external_dependency_values: ExternalDependencyValues,
        field_name: SmolStr,
        field_plan: &'a FieldPlan<'a>,
    },
}

impl<'a> IsInternalDependenciesOf<'a> {
    pub fn as_object_field_list_of_objects(
        &self,
    ) -> &IsInternalDependenciesOfObjectFieldListOfObjects<'a> {
        match self {
            Self::ObjectFieldListOfObjects(list) => list,
            _ => panic!("expected object field list of objects"),
        }
    }
}

pub struct IsInternalDependenciesOfObjectFieldListOfObjects<'a> {
    pub parent_object_index: IndexInProduced,
    pub index_of_field_in_object: usize,
    pub populator: &'a PopulatorList,
    pub external_dependency_values: ExternalDependencyValues,
    pub field_name: SmolStr,
    pub field_plan: &'a FieldPlan<'a>,
}
