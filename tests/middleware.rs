mod retry_after_middleware {
    use std::sync::Mutex;
    use std::time::{Duration, SystemTime};

    use assert_matches::assert_matches;
    use http::{Method, StatusCode};
    use reessaie::{RetryAfterMiddleware, RetryAfterPolicy};
    use reqwest::Client;
    use reqwest_middleware::ClientBuilder;
    use reqwest_retry::Jitter;
    use reqwest_retry::policies::ExponentialBackoff;
    use rstest::rstest;
    use tracing_test::traced_test;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

    #[derive(Debug)]
    struct ThrottledResponse {
        throttled_count: Mutex<u64>,
        throttling_response: ResponseTemplate,
        final_response: ResponseTemplate,
    }

    impl ThrottledResponse {
        fn new(
            throttled_count: u64,
            throttling_response: ResponseTemplate,
            final_response: ResponseTemplate,
        ) -> Self {
            Self {
                throttled_count: Mutex::new(throttled_count),
                throttling_response,
                final_response,
            }
        }
    }

    impl Respond for ThrottledResponse {
        fn respond(&self, _request: &Request) -> ResponseTemplate {
            let mut lock = self.throttled_count.lock().unwrap();
            if *lock > 0 {
                *lock -= 1;
                self.throttling_response.clone()
            } else {
                self.final_response.clone()
            }
        }
    }

    #[rstest]
    #[case::no_throttling(0, ResponseTemplate::new(StatusCode::OK), StatusCode::OK, Duration::ZERO)]
    #[case::one_throttling_without_retry_after_header(
        1,
        ResponseTemplate::new(StatusCode::TOO_MANY_REQUESTS),
        StatusCode::OK,
        Duration::from_millis(500)
    )]
    #[case::one_throttling_with_retry_after_header(
        1,
        ResponseTemplate::new(StatusCode::TOO_MANY_REQUESTS).append_header("Retry-After", "1"),
        StatusCode::OK,
        Duration::from_secs(1),
    )]
    #[case::too_many_throttlings(
        2,
        ResponseTemplate::new(StatusCode::TOO_MANY_REQUESTS),
        StatusCode::TOO_MANY_REQUESTS,
        Duration::from_millis(500)
    )]
    #[tokio::test]
    #[traced_test]
    async fn with(
        #[case] throttled_count: u64,
        #[case] throttling_response: ResponseTemplate,
        #[case] expected_status_code: StatusCode,
        #[case] expected_wait_time: Duration,
    ) {
        let mock_server = MockServer::start().await;

        Mock::given(method(Method::GET))
            .and(path("/"))
            .respond_with(ThrottledResponse::new(
                throttled_count,
                throttling_response,
                ResponseTemplate::new(StatusCode::OK),
            ))
            .expect(if throttled_count == 0 { 1 } else { 2 })
            .mount(&mock_server)
            .await;

        let retry_policy = ExponentialBackoff::builder()
            .retry_bounds(Duration::from_millis(500), Duration::from_millis(500))
            .jitter(Jitter::None)
            .build_with_max_retries(1);
        let retry_policy = RetryAfterPolicy::with_policy(retry_policy);

        let client = Client::new();
        let client = ClientBuilder::new(client)
            .with(RetryAfterMiddleware::new_with_policy(retry_policy))
            .build();

        let start_time = SystemTime::now();
        let response = client.get(format!("{}/", mock_server.uri())).send().await;
        let end_time = SystemTime::now();

        assert_matches!(response, Ok(res) if res.status() == expected_status_code);
        let actual_wait_time = end_time.duration_since(start_time).unwrap();
        let wait_time_diff = actual_wait_time.abs_diff(expected_wait_time);
        assert!(wait_time_diff <= Duration::from_millis(100));
    }
}
