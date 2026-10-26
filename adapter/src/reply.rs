//! The Message Router replies an adapter sends: a success carrying data, or a refusal.

use eipscanne_rs::cip::message::data::CipDataOpt;
use eipscanne_rs::cip::message::response::{
    MessageRouterResponse, ResponseData, ResponseStatusCode,
};
use eipscanne_rs::cip::message::shared::{ServiceCode, ServiceContainer};

/// A successful reply to `service` carrying `data`
pub fn success(service: ServiceCode, data: CipDataOpt) -> MessageRouterResponse {
    MessageRouterResponse {
        service_container: ServiceContainer::new_response(service),
        response_data: ResponseData {
            status: ResponseStatusCode::Success,
            additional_status_size: 0,
            additional_status: vec![],
            data,
        },
    }
}

/// A reply refusing `service` with `status` and no data
pub fn error(service: ServiceCode, status: ResponseStatusCode) -> MessageRouterResponse {
    MessageRouterResponse {
        service_container: ServiceContainer::new_response(service),
        response_data: ResponseData {
            status,
            additional_status_size: 0,
            additional_status: vec![],
            data: CipDataOpt::Raw(vec![]),
        },
    }
}
