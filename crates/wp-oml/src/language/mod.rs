mod prelude;
pub use syntax::{
    NestedBinding,
    OmlKwGet,
    VarAccess,
    accessors::{ArrOperation, FieldRead, FieldTake, FieldTakeBuilder, ReadOptionBuilder},
    accessors::{CondAccessor, DirectAccessor, GenericAccessor, NestedAccessor},
    accessors::{SqlFnArg, SqlFnExpr},
    bindings::GenericBinding,
    conditions::{ArgsTakeAble, CompareExpress, LogicalExpression},
    evaluators::{
        BatchEvalExp, BatchEvalExpBuilder, BatchEvaluation, EvalExp, PreciseEvaluator,
        SingleEvalExp, SingleEvalExpBuilder,
    },
    functions::{
        Base64Decode, Base64Encode, BuiltinFunction, Dumb, EncodeType, FUN_NOW_DATE, FUN_NOW_HOUR,
        FUN_NOW_TIME, FunOperation, Get, HtmlEscape, HtmlUnescape, Ip4ToInt, JsonEscape,
        JsonUnescape, NowDate, NowHour, NowTime, Nth, PIPE_BASE64_DECODE, PIPE_BASE64_ENCODE,
        PIPE_GET, PIPE_HTML_ESCAPE, PIPE_HTML_UNESCAPE, PIPE_IP4_TO_INT, PIPE_JSON_ESCAPE,
        PIPE_JSON_UNESCAPE, PIPE_NTH, PIPE_PATH, PIPE_SKIP_EMPTY, PIPE_STR_ESCAPE, PIPE_TIME_TO_TS,
        PIPE_TIME_TO_TS_MS, PIPE_TIME_TO_TS_US, PIPE_TIME_TO_TS_ZONE, PIPE_TO_JSON, PIPE_TO_STR,
        PIPE_URL, PathGet, PathType, PipeFun, SkipEmpty, StrEscape, TimeStampUnit, TimeToTs,
        TimeToTsMs, TimeToTsUs, TimeToTsZone, ToJson, ToStr, UrlGet, UrlType,
    },
    //lib_prm::LookupQuery,
    operations::{
        FmtOperation, MapOperation, MatchAble, MatchCase, MatchCond, MatchCondition,
        MatchOperation, MatchSource, PiPeOperation, RecordOperation, RecordOperationBuilder,
        SqlQuery,
    },
};
pub use types::model::DataModel;
pub use types::model::ObjModel;
pub use types::model::StubModel;
pub use types::target::{BatchEvalTarget, EvaluationTarget, EvaluationTargetBuilder};
mod syntax;
mod types;
pub const DCT_GET: &str = "get";
pub const DCT_OPTION: &str = "option";
pub const OML_CRATE_IN: &str = "in";
