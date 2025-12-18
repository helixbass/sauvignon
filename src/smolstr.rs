use smol_str::{SmolStr, ToSmolStr};
use sqlx::{
    encode::IsNull,
    error::BoxDynError,
    postgres::{PgArgumentBuffer, PgHasArrayType, PgTypeInfo, PgValueRef},
    Decode, Encode, Postgres, Type,
};

pub struct SmolStrSqlx(pub SmolStr);

impl Type<Postgres> for SmolStrSqlx {
    fn type_info() -> PgTypeInfo {
        <&'_ str as Type<Postgres>>::type_info()
    }

    fn compatible(ty: &PgTypeInfo) -> bool {
        <&'_ str as Type<Postgres>>::compatible(ty)
    }
}

impl PgHasArrayType for SmolStrSqlx {
    fn array_type_info() -> PgTypeInfo {
        <&'_ str as PgHasArrayType>::array_type_info()
    }

    fn array_compatible(ty: &PgTypeInfo) -> bool {
        <&'_ str as PgHasArrayType>::array_compatible(ty)
    }
}

impl Encode<'_, Postgres> for SmolStrSqlx {
    fn encode_by_ref(&self, buf: &mut PgArgumentBuffer) -> Result<IsNull, BoxDynError> {
        <&'_ str as Encode<'_, Postgres>>::encode_by_ref(&&*self.0, buf)
    }
}

impl Decode<'_, Postgres> for SmolStrSqlx {
    fn decode(value: PgValueRef<'_>) -> Result<Self, BoxDynError> {
        Ok(Self(value.as_str()?.to_smolstr()))
    }
}
