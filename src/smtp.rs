use samotop::model::controll::{TlsConfig, TlsIdFile, TlsMode};
use samotop::server::SamotopBuilder;
use samotop::service::session::StatefulSessionService;
use samotop::service::tcp::SamotopService;

/// Returns a TlsConfig that doesn't use TLS
pub fn tls_config_none() -> TlsConfig {
    TlsConfig {
        mode: TlsMode::Disabled,
        id: TlsIdFile {
            file: "notafile.bin".into(),
            password: None,
        },
    }
}

pub fn wrap_mailer_service<S>(
    mailer_service: S,
) -> SamotopBuilder<SamotopService<StatefulSessionService<S>>> {
    // Wrap the mailer service in a stateful SMTP session
    let custom_session_svc = StatefulSessionService::new(mailer_service);

    // TODO: allow the option for TLS
    let tls_conf = tls_config_none();

    // Wrap the stateful SMTP session in a TCP service
    let custom_svc = SamotopService::new(custom_session_svc, tls_conf);

    // Wraps the custom service in a samotop builder
    samotop::builder().with(custom_svc)
}
