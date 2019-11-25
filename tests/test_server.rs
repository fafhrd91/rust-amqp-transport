use actix_amqp::server::{self, errors};
use actix_amqp::{self, sasl, Configuration};
use actix_codec::{AsyncRead, AsyncWrite};
use actix_connect::{default_connector, TcpConnector};
use actix_service::{factory_fn_cfg, IntoServiceFactory, Service, ServiceFactory};
use actix_testing::{block_on, TestServer};
use futures::future::{err, Ready};
use futures::Future;
use http::{HttpTryFrom, Uri};

fn server(
    link: &server::Link<()>,
) -> impl Future<
    Output = Result<
        Box<
            dyn Service<
                    Request = server::Message<()>,
                    Response = server::Outcome,
                    Error = errors::AmqpError,
                    Future = Ready<Result<server::Message<()>, server::Outcome>>,
                > + 'static,
        >,
        errors::LinkError,
    >,
> {
    println!("OPEN LINK: {:?}", link);
    err(errors::LinkError::force_detach().description("unimplemented"))
}

#[test]
fn test_simple() -> std::io::Result<()> {
    std::env::set_var(
        "RUST_LOG",
        "actix_codec=info,actix_server=trace,actix_connector=trace,amqp_transport=trace",
    );
    env_logger::init();

    block_on(async {
        let srv = TestServer::with(|| {
            server::Server::new(
                server::Handshake::new(|conn: server::Connect<_>| {
                    async move {
                        let conn = conn.open().await.unwrap();
                        Ok::<_, errors::AmqpError>(conn.ack(()))
                    }
                })
                .sasl(server::sasl::no_sasl()),
            )
            .finish(
                server::App::<()>::new()
                    .service("test", factory_fn_cfg(server))
                    .finish(),
            )
        });

        let uri = Uri::try_from(format!("amqp://{}:{}", srv.host(), srv.port())).unwrap();
        let mut sasl_srv = sasl::connect_service(default_connector());
        let req = sasl::SaslConnect {
            uri,
            config: Configuration::default(),
            time: None,
            auth: sasl::SaslAuth {
                authz_id: "".to_string(),
                authn_id: "user1".to_string(),
                password: "password1".to_string(),
            },
        };
        let res = sasl_srv.call(req).await;
        println!("E: {:?}", res.err());

        Ok(())
    })
}

async fn sasl_auth<Io: AsyncRead + AsyncWrite>(
    auth: server::Sasl<Io>,
) -> Result<server::ConnectAck<Io, ()>, server::errors::ServerError<()>> {
    let init = auth
        .mechanism("PLAIN")
        .mechanism("ANONYMOUS")
        .mechanism("MSSBCBS")
        .mechanism("AMQPCBS")
        .init()
        .await?;

    if init.mechanism() == "PLAIN" {
        if let Some(resp) = init.initial_response() {
            if resp == b"\0user1\0password1" {
                let succ = init.outcome(amqp_codec::protocol::SaslCode::Ok).await?;
                return Ok(succ.open().await?.ack(()));
            }
        }
    }

    let succ = init.outcome(amqp_codec::protocol::SaslCode::Auth).await?;
    Ok(succ.open().await?.ack(()))
}

#[test]
fn test_sasl() -> std::io::Result<()> {
    block_on(async {
        let srv = TestServer::with(|| {
            server::Server::new(
                server::Handshake::new(|conn: server::Connect<_>| {
                    async move {
                        let conn = conn.open().await.unwrap();
                        Ok::<_, errors::Error>(conn.ack(()))
                    }
                })
                .sasl(sasl_auth.into_factory().map_err(|e| e.into())),
            )
            .finish(
                server::App::<()>::new()
                    .service("test", factory_fn_cfg(server))
                    .finish(),
            )
        });

        let uri = Uri::try_from(format!("amqp://{}:{}", srv.host(), srv.port())).unwrap();
        let mut sasl_srv = sasl::connect_service(TcpConnector::new());

        let req = sasl::SaslConnect {
            uri,
            config: Configuration::default(),
            time: None,
            auth: sasl::SaslAuth {
                authz_id: "".to_string(),
                authn_id: "user1".to_string(),
                password: "password1".to_string(),
            },
        };
        let res = sasl_srv.call(req).await;
        println!("E: {:?}", res.err());

        Ok(())
    })
}
