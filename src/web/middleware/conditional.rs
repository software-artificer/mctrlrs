use actix_web::{body, dev};
use std::{future, pin, rc};

pub struct ConditionalMiddleware<Next, F>(rc::Rc<Next>, F);

impl<Next, F> ConditionalMiddleware<Next, F> {
    pub fn new(next: Next, predicate: F) -> Self {
        Self(rc::Rc::new(next), predicate)
    }
}

impl<S, B, Next, SNext, BNext, F> dev::Transform<S, dev::ServiceRequest>
    for ConditionalMiddleware<Next, F>
where
    F: Fn(&dev::ServiceRequest) -> bool + Clone + 'static,
    B: 'static,
    S: dev::Service<
            dev::ServiceRequest,
            Response = dev::ServiceResponse<B>,
            Error = actix_web::Error,
        > + 'static,
    Next: dev::Transform<InnerService<S>, dev::ServiceRequest, Transform = SNext>,
    Next::Future: 'static,
    SNext: dev::Service<
            actix_web::dev::ServiceRequest,
            Response = dev::ServiceResponse<BNext>,
            Error = actix_web::Error,
        > + 'static,
{
    type Response = dev::ServiceResponse<body::EitherBody<B, BNext>>;
    type Error = actix_web::Error;
    type InitError = Next::InitError;
    type Transform = InnerCondMiddleware<S, F, SNext>;
    type Future =
        pin::Pin<Box<dyn future::Future<Output = Result<Self::Transform, Self::InitError>>>>;

    fn new_transform(&self, service: S) -> Self::Future {
        let service = InnerService {
            service: rc::Rc::new(service),
        };
        let next_service = self.0.new_transform(service.clone());
        let predicate = self.1.clone();
        Box::pin(async move {
            Ok(InnerCondMiddleware::<S, F, SNext> {
                service,
                next: next_service.await?,
                predicate,
            })
        })
    }
}

pub struct InnerService<S> {
    service: rc::Rc<S>,
}

impl<S> Clone for InnerService<S> {
    fn clone(&self) -> Self {
        Self {
            service: self.service.clone(),
        }
    }
}

impl<S, B> dev::Service<dev::ServiceRequest> for InnerService<S>
where
    B: 'static,
    S: dev::Service<
            dev::ServiceRequest,
            Response = dev::ServiceResponse<B>,
            Error = actix_web::Error,
        > + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;

    dev::forward_ready!(service);

    fn call(&self, req: dev::ServiceRequest) -> Self::Future {
        self.service.call(req)
    }
}

pub struct InnerCondMiddleware<S, F, Next> {
    service: InnerService<S>,
    next: Next,
    predicate: F,
}

impl<S, B, Next, BNext, F> dev::Service<dev::ServiceRequest> for InnerCondMiddleware<S, F, Next>
where
    B: 'static,
    S: dev::Service<
            dev::ServiceRequest,
            Response = dev::ServiceResponse<B>,
            Error = actix_web::Error,
        > + 'static,
    Next: dev::Service<
            dev::ServiceRequest,
            Response = dev::ServiceResponse<BNext>,
            Error = actix_web::Error,
        > + 'static,
    F: Fn(&dev::ServiceRequest) -> bool,
    Next::Future: 'static,
{
    type Response = dev::ServiceResponse<body::EitherBody<B, BNext>>;
    type Error = actix_web::Error;
    type Future = pin::Pin<Box<dyn future::Future<Output = Result<Self::Response, Self::Error>>>>;

    dev::forward_ready!(service);

    fn call(&self, req: dev::ServiceRequest) -> Self::Future {
        if (self.predicate)(&req) {
            let res = self.next.call(req);
            Box::pin(async move { res.await.map(dev::ServiceResponse::map_into_right_body) })
        } else {
            let res = self.service.call(req);
            Box::pin(async move { res.await.map(dev::ServiceResponse::map_into_left_body) })
        }
    }
}
