use std::sync::OnceLock;
use time::{
    format_description::{self, FormatItem},
    OffsetDateTime, UtcOffset,
};

pub fn format_time(time: OffsetDateTime) -> String {
    static OFFSET: OnceLock<UtcOffset> = OnceLock::new();
    static FORMAT: OnceLock<Vec<FormatItem>> = OnceLock::new();
    let offset = *OFFSET.get_or_init(|| {
        UtcOffset::current_local_offset().unwrap_or(UtcOffset::from_hms(8, 0, 0).unwrap())
    });
    let format = &**FORMAT.get_or_init(|| {
        format_description::parse("[year]-[month]-[day]-[hour]-[minute]-[second].[subsecond]")
            .unwrap()
    });

    time.to_offset(offset).format(format).unwrap()
}
