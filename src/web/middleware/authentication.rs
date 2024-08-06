use actix_web::{body, dev, http, FromRequest};
use std::{future, marker, pin, rc};

pub trait AuthSession: FromRequest {
    type IsAuthenticatedError: actix_web::ResponseError + 'static;
    type SaveRedirectError: actix_web::ResponseError + 'static;

    fn is_authenticated(&self) -> Result<bool, Self::IsAuthenticatedError>;
    fn save_redirect(&self, location: String) -> Result<(), Self::SaveRedirectError>;
}

pub struct AuthMiddleware<A: AuthSession>(String, marker::PhantomData<A>);

impl<A: AuthSession> AuthMiddleware<A> {
    pub fn new<P: AsRef<str>>(login_path: P) -> Self {
        Self(login_path.as_ref().to_owned(), marker::PhantomData)
    }
}

impl<S, B, A> dev::Transform<S, dev::ServiceRequest> for AuthMiddleware<A>
where
    S: dev::Service<
        dev::ServiceRequest,
        Response = dev::ServiceResponse<B>,
        Error = actix_web::Error,
    >,
    S::Future: 'static,
    B: 'static,
    S: 'static,
    A: AuthSession,
{
    type Response = dev::ServiceResponse<body::EitherBody<B>>;
    type Error = actix_web::Error;
    type InitError = ();
    type Transform = InnerAuthMiddleware<S, A>;
    type Future = future::Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        future::ready(Ok(InnerAuthMiddleware {
            service: rc::Rc::new(service),
            login_path: self.0.clone(),
            auth_session: marker::PhantomData,
        }))
    }
}

pub struct InnerAuthMiddleware<S, A> {
    service: rc::Rc<S>,
    login_path: String,
    auth_session: marker::PhantomData<A>,
}

impl<S, B, A> dev::Service<dev::ServiceRequest> for InnerAuthMiddleware<S, A>
where
    S: dev::Service<
            dev::ServiceRequest,
            Response = dev::ServiceResponse<B>,
            Error = actix_web::Error,
        > + 'static,
    S::Future: 'static,
    B: 'static,
    A: AuthSession,
{
    type Response = dev::ServiceResponse<body::EitherBody<B>>;
    type Error = actix_web::Error;
    type Future = pin::Pin<Box<dyn future::Future<Output = Result<Self::Response, Self::Error>>>>;

    dev::forward_ready!(service);

    fn call(&self, mut req: dev::ServiceRequest) -> Self::Future {
        let svc = self.service.clone();
        let login_path = self.login_path.clone();

        Box::pin(async move {
            let session = {
                let (http_request, payload) = req.parts_mut();
                A::from_request(http_request, payload)
                    .await
                    .map_err(|err| err.into())?
            };

            let is_authenticated = session.is_authenticated()?;

            if !is_authenticated && req.path() != login_path {
                session.save_redirect(login_path.clone())?;

                let response = actix_web::HttpResponse::Found()
                    .insert_header((http::header::LOCATION, login_path))
                    .finish()
                    .map_into_right_body();

                let (http_request, _) = req.into_parts();

                Ok(dev::ServiceResponse::new(http_request, response))
            } else {
                svc.call(req)
                    .await
                    .map(dev::ServiceResponse::map_into_left_body)
            }
        })
    }
}
