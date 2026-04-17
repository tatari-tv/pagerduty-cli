use pagerduty_cli::client::{ApiError, PdClient};
use reqwest::StatusCode;
use serde_json::json;
use wiremock::matchers::{header, method, path, path_regex, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

// ---------------------------------------------------------------------------
// Helper: create a PdClient pointed at the mock server
// ---------------------------------------------------------------------------

async fn mock_client(server: &MockServer) -> PdClient {
    PdClient::new("test-token".to_string())
        .unwrap()
        .with_base_url(server.uri())
}

// ---------------------------------------------------------------------------
// Client: GET requests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn client_get_sends_auth_header() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/test"))
        .and(header("Authorization", "Token token=test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"ok": true})))
        .expect(1)
        .mount(&server)
        .await;

    let client = mock_client(&server).await;
    let resp = client.get("/test").await.unwrap();
    assert_eq!(resp["ok"], true);
}

#[tokio::test]
async fn client_get_sends_content_type() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/test"))
        .and(header("Content-Type", "application/json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"ok": true})))
        .expect(1)
        .mount(&server)
        .await;

    let client = mock_client(&server).await;
    client.get("/test").await.unwrap();
}

#[tokio::test]
async fn client_get_sends_accept_header() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/test"))
        .and(header("Accept", "application/vnd.pagerduty+json;version=2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"ok": true})))
        .expect(1)
        .mount(&server)
        .await;

    let client = mock_client(&server).await;
    client.get("/test").await.unwrap();
}

// ---------------------------------------------------------------------------
// Client: 204 No Content (DELETE trigger)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn client_delete_204_no_content_returns_null() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/incident_workflows/triggers/T1"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let client = mock_client(&server).await;
    let result = client.delete("/incident_workflows/triggers/T1").await.unwrap();
    assert_eq!(result, serde_json::Value::Null);
}

#[tokio::test]
async fn client_delete_200_with_body_returns_value() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/incident_workflows/WF1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"deleted": true})))
        .expect(1)
        .mount(&server)
        .await;

    let client = mock_client(&server).await;
    let result = client.delete("/incident_workflows/WF1").await.unwrap();
    assert_eq!(result["deleted"], true);
}

// ---------------------------------------------------------------------------
// Client: structured error parsing
// ---------------------------------------------------------------------------

#[tokio::test]
async fn client_error_response_extracts_message_and_details() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/test"))
        .respond_with(ResponseTemplate::new(400).set_body_json(json!({
            "error": {
                "message": "Invalid Input",
                "code": 2001,
                "errors": ["field 'name' is required", "field 'display_name' is required"]
            }
        })))
        .mount(&server)
        .await;

    let client = mock_client(&server).await;
    let err = client.post("/test", json!({})).await.unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("Invalid Input"));
    assert!(msg.contains("name"));
}

#[tokio::test]
async fn client_pcl_error_includes_doc_hint() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/incident_workflows/triggers"))
        .respond_with(ResponseTemplate::new(400).set_body_json(json!({
            "error": {
                "message": "Invalid condition",
                "errors": ["PCL parse error at position 5"]
            }
        })))
        .mount(&server)
        .await;

    let client = mock_client(&server).await;
    let err = client
        .post("/incident_workflows/triggers", json!({}))
        .await
        .unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("PCL reference"));
}

// ---------------------------------------------------------------------------
// Client: error handling
// ---------------------------------------------------------------------------

#[tokio::test]
async fn client_400_returns_error_with_body() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/bad"))
        .respond_with(ResponseTemplate::new(400).set_body_json(json!({
            "error": {
                "message": "Invalid Input",
                "code": 2001,
                "errors": ["name is required"]
            }
        })))
        .mount(&server)
        .await;

    let client = mock_client(&server).await;
    let err = client.get("/bad").await.unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("400"), "expected 400 in error: {}", msg);
    assert!(msg.contains("Invalid Input"), "expected error body in message: {}", msg);
}

#[tokio::test]
async fn client_404_returns_error() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/not-found"))
        .respond_with(ResponseTemplate::new(404).set_body_json(json!({
            "error": {
                "message": "Not Found",
                "code": 2100
            }
        })))
        .mount(&server)
        .await;

    let client = mock_client(&server).await;
    let err = client.get("/not-found").await.unwrap_err();
    assert!(err.to_string().contains("404"));
}

// ---------------------------------------------------------------------------
// Priorities: list
// ---------------------------------------------------------------------------

#[tokio::test]
async fn priorities_list_returns_pd_shaped_response() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path_regex(r"/priorities.*"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "priorities": [
                {"id": "P1ID", "name": "P1", "description": "Critical", "color": "red", "order": 1},
                {"id": "P2ID", "name": "P2", "description": "Major", "color": "orange", "order": 2},
                {"id": "P3ID", "name": "P3", "description": "Minor", "color": "yellow", "order": 3},
                {"id": "P4ID", "name": "P4", "description": "Triage", "color": "blue", "order": 4}
            ],
            "limit": 25,
            "offset": 0,
            "more": false
        })))
        .mount(&server)
        .await;

    let client = mock_client(&server).await;
    let resp = client.get("/priorities").await.unwrap();
    let priorities = resp["priorities"].as_array().unwrap();
    assert_eq!(priorities.len(), 4);
    assert_eq!(priorities[0]["name"], "P1");
    assert_eq!(priorities[3]["name"], "P4");
}

// ---------------------------------------------------------------------------
// Incident types: list and create
// ---------------------------------------------------------------------------

#[tokio::test]
async fn incident_types_list() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path_regex(r"/incidents/types.*"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "incident_types": [
                {
                    "id": "IT001",
                    "name": "default",
                    "display_name": "Default",
                    "description": "Default incident type",
                    "enabled": true
                },
                {
                    "id": "IT002",
                    "name": "managed_incident",
                    "display_name": "Managed Incident",
                    "description": "Full incident response",
                    "enabled": true
                }
            ],
            "limit": 25,
            "offset": 0,
            "more": false
        })))
        .mount(&server)
        .await;

    let client = mock_client(&server).await;
    let resp = client.get("/incidents/types").await.unwrap();
    let types = resp["incident_types"].as_array().unwrap();
    assert_eq!(types.len(), 2);
    assert_eq!(types[1]["display_name"], "Managed Incident");
}

#[tokio::test]
async fn incident_type_create() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/incidents/types"))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "incident_type": {
                "id": "IT003",
                "name": "business_incident",
                "display_name": "Business Incident",
                "description": "Business-level incident",
                "enabled": true
            }
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = mock_client(&server).await;
    let body = json!({
        "incident_type": {
            "name": "business_incident",
            "display_name": "Business Incident",
            "description": "Business-level incident"
        }
    });
    let resp = client.post("/incidents/types", body).await.unwrap();
    assert_eq!(resp["incident_type"]["id"], "IT003");
}

// ---------------------------------------------------------------------------
// Incident workflows: list, get with steps, create, update, delete
// ---------------------------------------------------------------------------

#[tokio::test]
async fn workflows_list() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/incident_workflows"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "incident_workflows": [
                {"id": "WF1", "name": "Managed Incident Response", "is_enabled": true},
                {"id": "WF2", "name": "Incident Visibility", "is_enabled": true},
                {"id": "WF3", "name": "Auto-Manage P1", "is_enabled": false}
            ],
            "limit": 25,
            "offset": 0,
            "more": false
        })))
        .mount(&server)
        .await;

    let client = mock_client(&server).await;
    let resp = client.get("/incident_workflows").await.unwrap();
    let workflows = resp["incident_workflows"].as_array().unwrap();
    assert_eq!(workflows.len(), 3);
}

#[tokio::test]
async fn workflow_get_with_steps() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path_regex(r"/incident_workflows/WF1.*"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "incident_workflow": {
                "id": "WF1",
                "name": "Managed Incident Response",
                "description": "Full response for managed incidents",
                "is_enabled": true,
                "steps": [
                    {
                        "id": "S1",
                        "name": "Create Slack Channel",
                        "action_configuration": {
                            "action_id": "pagerduty.slack.create-dedicated-channel",
                            "inputs": [
                                {"name": "channel_name", "value": "incident-{{incident.id}}"}
                            ]
                        }
                    },
                    {
                        "id": "S2",
                        "name": "Set Channel Topic",
                        "action_configuration": {
                            "action_id": "pagerduty.slack.set-channel-topic",
                            "inputs": [
                                {"name": "topic", "value": "{{incident.title}}"}
                            ]
                        }
                    }
                ]
            }
        })))
        .mount(&server)
        .await;

    let client = mock_client(&server).await;
    let resp = client.get("/incident_workflows/WF1?include[]=steps").await.unwrap();
    let wf = &resp["incident_workflow"];
    assert_eq!(wf["name"], "Managed Incident Response");
    assert_eq!(wf["steps"].as_array().unwrap().len(), 2);
    assert_eq!(
        wf["steps"][0]["action_configuration"]["action_id"],
        "pagerduty.slack.create-dedicated-channel"
    );
}

#[tokio::test]
async fn workflow_create_from_body() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/incident_workflows"))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "incident_workflow": {
                "id": "WF_NEW",
                "name": "Auto-Manage P1",
                "description": "Auto-set managed for P1s",
                "is_enabled": false,
                "steps": [{
                    "id": "S1",
                    "name": "Set Incident Type",
                    "action_configuration": {
                        "action_id": "pagerduty.incident-management.update-incident-type",
                        "inputs": [{"name": "incident_type", "value": "Managed Incident"}]
                    }
                }]
            }
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = mock_client(&server).await;
    let body = json!({
        "incident_workflow": {
            "name": "Auto-Manage P1",
            "description": "Auto-set managed for P1s",
            "is_enabled": false,
            "steps": [{
                "name": "Set Incident Type",
                "action_configuration": {
                    "action_id": "pagerduty.incident-management.update-incident-type",
                    "inputs": [{"name": "incident_type", "value": "Managed Incident"}]
                }
            }]
        }
    });
    let resp = client.post("/incident_workflows", body).await.unwrap();
    assert_eq!(resp["incident_workflow"]["id"], "WF_NEW");
    assert_eq!(resp["incident_workflow"]["name"], "Auto-Manage P1");
}

#[tokio::test]
async fn workflow_update() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/incident_workflows/WF1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "incident_workflow": {
                "id": "WF1",
                "name": "Updated Name",
                "is_enabled": true
            }
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = mock_client(&server).await;
    let body = json!({"incident_workflow": {"name": "Updated Name", "is_enabled": true}});
    let resp = client.put("/incident_workflows/WF1", body).await.unwrap();
    assert_eq!(resp["incident_workflow"]["name"], "Updated Name");
}

#[tokio::test]
async fn workflow_delete() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/incident_workflows/WF1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
        .expect(1)
        .mount(&server)
        .await;

    let client = mock_client(&server).await;
    client.delete("/incident_workflows/WF1").await.unwrap();
}

// ---------------------------------------------------------------------------
// Triggers: CRUD
// ---------------------------------------------------------------------------

#[tokio::test]
async fn triggers_list() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/incident_workflows/triggers"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "triggers": [
                {
                    "id": "T1",
                    "trigger_type": "incident_type",
                    "incident_types": ["IT002"],
                    "workflow": {"id": "WF1", "type": "workflow"}
                },
                {
                    "id": "T2",
                    "trigger_type": "conditional",
                    "condition": "incident.priority matches 'P1'",
                    "workflow": {"id": "WF3", "type": "workflow"}
                }
            ]
        })))
        .mount(&server)
        .await;

    let client = mock_client(&server).await;
    let resp = client.get("/incident_workflows/triggers").await.unwrap();
    let triggers = resp["triggers"].as_array().unwrap();
    assert_eq!(triggers.len(), 2);
    assert_eq!(triggers[0]["trigger_type"], "incident_type");
    assert_eq!(triggers[1]["trigger_type"], "conditional");
}

#[tokio::test]
async fn trigger_create_conditional() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/incident_workflows/triggers"))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "trigger": {
                "id": "T_NEW",
                "trigger_type": "conditional",
                "condition": "incident.priority matches 'P1'",
                "workflow": {"id": "WF3", "type": "workflow"}
            }
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = mock_client(&server).await;
    let body = json!({
        "trigger": {
            "trigger_type": "conditional",
            "condition": "incident.priority matches 'P1'",
            "workflow": {"id": "WF3", "type": "workflow"}
        }
    });
    let resp = client.post("/incident_workflows/triggers", body).await.unwrap();
    assert_eq!(resp["trigger"]["id"], "T_NEW");
    assert_eq!(resp["trigger"]["trigger_type"], "conditional");
}

#[tokio::test]
async fn trigger_create_incident_type() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/incident_workflows/triggers"))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "trigger": {
                "id": "T_IT",
                "trigger_type": "incident_type",
                "incident_types": ["IT002"],
                "workflow": {"id": "WF1", "type": "workflow"}
            }
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = mock_client(&server).await;
    let body = json!({
        "trigger": {
            "trigger_type": "incident_type",
            "incident_types": ["IT002"],
            "workflow": {"id": "WF1", "type": "workflow"}
        }
    });
    let resp = client.post("/incident_workflows/triggers", body).await.unwrap();
    assert_eq!(resp["trigger"]["trigger_type"], "incident_type");
}

#[tokio::test]
async fn trigger_delete() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/incident_workflows/triggers/T1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
        .expect(1)
        .mount(&server)
        .await;

    let client = mock_client(&server).await;
    client.delete("/incident_workflows/triggers/T1").await.unwrap();
}

#[tokio::test]
async fn trigger_create_for_service() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/incident_workflows/triggers/T1/services"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "service": {"id": "SVC1", "type": "service_reference"}
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = mock_client(&server).await;
    let body = json!({"service": {"id": "SVC1", "type": "service_reference"}});
    let resp = client
        .post("/incident_workflows/triggers/T1/services", body)
        .await
        .unwrap();
    assert_eq!(resp["service"]["id"], "SVC1");
}

// ---------------------------------------------------------------------------
// Actions: discovery
// ---------------------------------------------------------------------------

#[tokio::test]
async fn actions_list() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/incident_workflows/actions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "actions": [
                {
                    "id": "pagerduty.slack.create-dedicated-channel",
                    "name": "Create Slack Channel",
                    "domain_name": "pagerduty",
                    "package_name": "slack",
                    "function_name": "create_dedicated_channel",
                    "inputs": [
                        {"name": "channel_name", "type": "string", "required": true}
                    ],
                    "outputs": [
                        {"name": "channel_id", "type": "string"}
                    ]
                },
                {
                    "id": "pagerduty.slack.send-message",
                    "name": "Send Message",
                    "domain_name": "pagerduty",
                    "package_name": "slack",
                    "function_name": "send_message",
                    "inputs": [
                        {"name": "channel", "type": "string", "required": true},
                        {"name": "message", "type": "string", "required": true}
                    ],
                    "outputs": []
                }
            ]
        })))
        .mount(&server)
        .await;

    let client = mock_client(&server).await;
    let resp = client.get("/incident_workflows/actions").await.unwrap();
    let actions = resp["actions"].as_array().unwrap();
    assert_eq!(actions.len(), 2);
    assert_eq!(actions[0]["id"], "pagerduty.slack.create-dedicated-channel");
}

#[tokio::test]
async fn action_get_with_schema() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(
            "/incident_workflows/actions/pagerduty.slack.create-dedicated-channel",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "action": {
                "id": "pagerduty.slack.create-dedicated-channel",
                "name": "Create Slack Channel",
                "description": "Creates a dedicated Slack channel for the incident",
                "domain_name": "pagerduty",
                "package_name": "slack",
                "function_name": "create_dedicated_channel",
                "inputs": [
                    {
                        "name": "channel_name",
                        "type": "string",
                        "description": "Name of the channel to create",
                        "required": true
                    }
                ],
                "outputs": [
                    {
                        "name": "channel_id",
                        "type": "string",
                        "description": "ID of the created channel"
                    }
                ]
            }
        })))
        .mount(&server)
        .await;

    let client = mock_client(&server).await;
    let resp = client
        .get("/incident_workflows/actions/pagerduty.slack.create-dedicated-channel")
        .await
        .unwrap();
    assert_eq!(resp["action"]["id"], "pagerduty.slack.create-dedicated-channel");
    assert!(!resp["action"]["inputs"].as_array().unwrap().is_empty());
}

// ---------------------------------------------------------------------------
// Pagination
// ---------------------------------------------------------------------------

#[tokio::test]
async fn client_get_all_paginates() {
    let server = MockServer::start().await;

    // Page 1: more=true
    Mock::given(method("GET"))
        .and(path("/incident_workflows"))
        .and(query_param("offset", "0"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "incident_workflows": [
                {"id": "WF1", "name": "Workflow 1"},
                {"id": "WF2", "name": "Workflow 2"}
            ],
            "limit": 25,
            "offset": 0,
            "more": true
        })))
        .expect(1)
        .mount(&server)
        .await;

    // Page 2: more=false
    Mock::given(method("GET"))
        .and(path("/incident_workflows"))
        .and(query_param("offset", "25"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "incident_workflows": [
                {"id": "WF3", "name": "Workflow 3"}
            ],
            "limit": 25,
            "offset": 25,
            "more": false
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = mock_client(&server).await;
    let all = client
        .get_all("/incident_workflows", "incident_workflows")
        .await
        .unwrap();
    assert_eq!(all.len(), 3);
    assert_eq!(all[0]["name"], "Workflow 1");
    assert_eq!(all[2]["name"], "Workflow 3");
}

// ---------------------------------------------------------------------------
// ApiError: errors from failed responses are downcastable to inspect status
// ---------------------------------------------------------------------------

#[tokio::test]
async fn api_error_is_downcastable_for_404() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/missing"))
        .respond_with(ResponseTemplate::new(404).set_body_json(json!({
            "error": {"message": "Not found"}
        })))
        .mount(&server)
        .await;

    let client = mock_client(&server).await;
    let err = client.get("/missing").await.unwrap_err();
    let api_err = err.downcast_ref::<ApiError>().expect("expected ApiError");
    assert_eq!(api_err.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn api_error_display_matches_legacy_format() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/bad"))
        .respond_with(ResponseTemplate::new(400).set_body_json(json!({
            "error": {"message": "Invalid Input", "errors": ["name is required"]}
        })))
        .mount(&server)
        .await;

    let client = mock_client(&server).await;
    let err = client.get("/bad").await.unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("400"));
    assert!(msg.contains("Invalid Input"));
    assert!(msg.contains("name is required"));
}

// ---------------------------------------------------------------------------
// try_get: 404 becomes Ok(None); other errors propagate
// ---------------------------------------------------------------------------

#[tokio::test]
async fn try_get_returns_none_on_404() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/incidents/types/nonexistent"))
        .respond_with(ResponseTemplate::new(404).set_body_json(json!({
            "error": {"message": "Not Found"}
        })))
        .mount(&server)
        .await;

    let client = mock_client(&server).await;
    let result = client.try_get("/incidents/types/nonexistent").await.unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn try_get_returns_some_on_200() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/incidents/types/IT001"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "incident_type": {"id": "IT001", "name": "default"}
        })))
        .mount(&server)
        .await;

    let client = mock_client(&server).await;
    let result = client.try_get("/incidents/types/IT001").await.unwrap();
    let v = result.expect("expected Some on 200");
    assert_eq!(v["incident_type"]["id"], "IT001");
}

#[tokio::test]
async fn try_get_plus_list_implements_display_name_fallback() {
    // End-to-end shape that incident-type get uses: direct GET 404s on the
    // display name, list scan finds the type by display_name.
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/incidents/types/Managed%20Incident"))
        .respond_with(ResponseTemplate::new(404).set_body_json(json!({
            "error": {"message": "Not Found"}
        })))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/incidents/types"))
        .and(query_param("offset", "0"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "incident_types": [
                {"id": "IT001", "name": "incident_default", "display_name": "Base Incident", "enabled": true},
                {"id": "IT002", "name": "managed_incident", "display_name": "Managed Incident", "enabled": true}
            ],
            "more": false
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = mock_client(&server).await;
    let direct = client.try_get("/incidents/types/Managed Incident").await.unwrap();
    assert!(direct.is_none(), "direct lookup should 404");

    let all = client.get_all("/incidents/types", "incident_types").await.unwrap();
    let matched: Vec<&serde_json::Value> = all
        .iter()
        .filter(|t| {
            t.get("display_name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .eq_ignore_ascii_case("Managed Incident")
        })
        .collect();
    assert_eq!(matched.len(), 1);
    assert_eq!(matched[0]["id"], "IT002");
}

#[tokio::test]
async fn field_list_display_name_fallback_fetches_by_resolved_id() {
    // Mirrors the incident-type field list flow: direct GET on display name
    // 404s, list scan finds the real ID, then custom_fields is fetched using
    // that ID. The bug before v0.2.2 was that field list passed the raw
    // display name straight to the URL, 404ing instead of resolving.
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/incidents/types/Managed%20Incident"))
        .respond_with(ResponseTemplate::new(404).set_body_json(json!({
            "error": {"message": "Not Found"}
        })))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/incidents/types"))
        .and(query_param("offset", "0"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "incident_types": [
                {"id": "IT002", "name": "managed_incident", "display_name": "Managed Incident", "enabled": true}
            ],
            "more": false
        })))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/incidents/types/IT002/custom_fields"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "fields": [
                {"id": "F1", "name": "slack_channel_url", "display_name": "Slack Channel URL"}
            ]
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = mock_client(&server).await;

    // Sequence: try_get 404, list fallback, extract ID, fetch fields by ID.
    assert!(
        client
            .try_get("/incidents/types/Managed Incident")
            .await
            .unwrap()
            .is_none()
    );
    let all = client.get_all("/incidents/types", "incident_types").await.unwrap();
    let id = all[0]["id"].as_str().unwrap();
    assert_eq!(id, "IT002");
    let fields_resp = client
        .get(&format!("/incidents/types/{}/custom_fields", id))
        .await
        .unwrap();
    assert_eq!(fields_resp["fields"][0]["id"], "F1");
}

#[tokio::test]
async fn try_get_propagates_non_404_errors() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/unauth"))
        .respond_with(ResponseTemplate::new(401).set_body_json(json!({
            "error": {"message": "Unauthorized"}
        })))
        .mount(&server)
        .await;

    let client = mock_client(&server).await;
    let err = client.try_get("/unauth").await.unwrap_err();
    let api_err = err.downcast_ref::<ApiError>().expect("expected ApiError");
    assert_eq!(api_err.status, StatusCode::UNAUTHORIZED);
}

// ---------------------------------------------------------------------------
// get_all_no_offset: /incident_workflows/triggers and /incident_workflows/actions
// reject ?offset=N but accept ?limit=N. Single large-page fetch, no retry.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn get_all_no_offset_succeeds_without_offset_param() {
    let server = MockServer::start().await;

    // Reject any request that includes offset
    Mock::given(method("GET"))
        .and(path("/incident_workflows/triggers"))
        .and(query_param("offset", "0"))
        .respond_with(ResponseTemplate::new(400).set_body_json(json!({
            "error": {"message": "offset is not allowed"}
        })))
        .mount(&server)
        .await;

    // Accept limit-only requests
    Mock::given(method("GET"))
        .and(path("/incident_workflows/triggers"))
        .and(query_param("limit", "100"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "triggers": [
                {"id": "T1", "trigger_type": "conditional"},
                {"id": "T2", "trigger_type": "manual"}
            ],
            "limit": 100,
            "more": false
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = mock_client(&server).await;
    let all = client
        .get_all_no_offset("/incident_workflows/triggers", "triggers")
        .await
        .unwrap();
    assert_eq!(all.len(), 2);
    assert_eq!(all[0]["id"], "T1");
}

#[tokio::test]
async fn get_all_no_offset_appends_to_existing_query_string() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/incident_workflows/actions"))
        .and(query_param("query", "slack"))
        .and(query_param("limit", "100"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "actions": [{"id": "pagerduty.slack.send-message"}]
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = mock_client(&server).await;
    let all = client
        .get_all_no_offset("/incident_workflows/actions?query=slack", "actions")
        .await
        .unwrap();
    assert_eq!(all.len(), 1);
}

#[tokio::test]
async fn get_all_no_offset_handles_empty_response() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/incident_workflows/triggers"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "triggers": [],
            "limit": 200,
            "more": false
        })))
        .mount(&server)
        .await;

    let client = mock_client(&server).await;
    let all = client
        .get_all_no_offset("/incident_workflows/triggers", "triggers")
        .await
        .unwrap();
    assert!(all.is_empty());
}

// ---------------------------------------------------------------------------
// REST passthrough
// ---------------------------------------------------------------------------

#[tokio::test]
async fn raw_passthrough_get() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/some/arbitrary/path"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"data": "raw"})))
        .mount(&server)
        .await;

    let client = mock_client(&server).await;
    let resp = client.raw("GET", "/some/arbitrary/path", None).await.unwrap();
    assert_eq!(resp["data"], "raw");
}

#[tokio::test]
async fn raw_passthrough_post_with_body() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/some/path"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"created": true})))
        .mount(&server)
        .await;

    let client = mock_client(&server).await;
    let body = json!({"key": "value"});
    let resp = client.raw("POST", "/some/path", Some(body)).await.unwrap();
    assert_eq!(resp["created"], true);
}

// ---------------------------------------------------------------------------
// Workflow YAML definition roundtrip through library types
// ---------------------------------------------------------------------------

#[tokio::test]
async fn workflow_definition_yaml_roundtrip() {
    use pagerduty_cli::resources::incident::workflows::{
        InputYaml, StepYaml, TriggerYaml, WorkflowDefinition, WorkflowYaml,
    };

    let def = WorkflowDefinition {
        workflow: WorkflowYaml {
            name: "Auto-Manage P1".to_string(),
            description: Some("Auto-set managed for P1s".to_string()),
            is_enabled: false,
            steps: vec![StepYaml {
                name: "Set Incident Type".to_string(),
                description: None,
                action_id: "pagerduty.incident-management.update-incident-type".to_string(),
                inputs: vec![InputYaml {
                    name: "incident_type".to_string(),
                    value: "Managed Incident".to_string(),
                }],
            }],
        },
        trigger: Some(TriggerYaml {
            trigger_type: "conditional".to_string(),
            condition: Some("incident.priority matches 'P1'".to_string()),
            incident_types: None,
        }),
    };

    // Verify YAML serialization round-trips correctly
    let yaml = serde_yaml::to_string(&def).unwrap();
    let parsed: WorkflowDefinition = serde_yaml::from_str(&yaml).unwrap();
    assert_eq!(parsed.workflow.name, "Auto-Manage P1");
    assert_eq!(parsed.workflow.steps.len(), 1);
    assert_eq!(
        parsed.workflow.steps[0].action_id,
        "pagerduty.incident-management.update-incident-type"
    );
    assert!(parsed.trigger.is_some());
    let trigger = parsed.trigger.unwrap();
    assert_eq!(trigger.trigger_type, "conditional");
    assert_eq!(trigger.condition.as_deref(), Some("incident.priority matches 'P1'"));
}

// ---------------------------------------------------------------------------
// Workflow YAML files load and produce valid API bodies
// ---------------------------------------------------------------------------

#[tokio::test]
async fn all_workflow_yaml_files_produce_valid_api_payloads() {
    use pagerduty_cli::resources::incident::workflows::WorkflowDefinition;
    use std::fs;

    let workflow_dir = format!("{}/workflows", env!("CARGO_MANIFEST_DIR"));
    let files = [
        "wf1-managed-incident-response.yml",
        "wf2-incident-visibility.yml",
        "wf3-auto-manage-p1.yml",
        "wf4a-auto-manage-p1-escalation.yml",
        "wf4b-priority-changed.yml",
    ];

    for file in files {
        let path = format!("{}/{}", workflow_dir, file);
        let content = fs::read_to_string(&path).unwrap_or_else(|e| panic!("Failed to read {}: {}", path, e));
        let def: WorkflowDefinition =
            serde_yaml::from_str(&content).unwrap_or_else(|e| panic!("Failed to parse {}: {}", path, e));

        // Verify required fields
        assert!(!def.workflow.name.is_empty(), "{} has empty name", file);
        assert!(!def.workflow.steps.is_empty(), "{} has no steps", file);
        assert!(!def.workflow.is_enabled, "{} should default to disabled", file);
        assert!(def.trigger.is_some(), "{} missing trigger", file);

        // Verify each step has required fields
        for step in &def.workflow.steps {
            assert!(!step.name.is_empty(), "{}: step has empty name", file);
            assert!(
                !step.action_id.is_empty(),
                "{}: step '{}' has empty action_id",
                file,
                step.name
            );
        }

        // Verify JSON serialization works (simulates API body generation)
        let json = serde_json::to_value(&def).unwrap();
        assert!(json.get("workflow").is_some(), "{} missing workflow key in JSON", file);
    }
}

// ---------------------------------------------------------------------------
// Shadow-workflow fallback scenarios via get_all_no_offset on triggers.
// Exercises the shape export() relies on, without duplicating the handler.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn shadow_scan_finds_exactly_one_match() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/incident_workflows/triggers"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "triggers": [
                {
                    "id": "T1",
                    "trigger_type": "incident_type",
                    "workflow": {"id": "WF_REAL", "name": "Managed Incident Response"}
                },
                {
                    "id": "T2",
                    "trigger_type": "conditional",
                    "workflow": {"id": "WF_OTHER", "name": "Some Other Workflow"}
                }
            ],
            "more": false
        })))
        .mount(&server)
        .await;

    let client = mock_client(&server).await;
    let triggers = client
        .get_all_no_offset("/incident_workflows/triggers", "triggers")
        .await
        .unwrap();
    let matched: Vec<&str> = triggers
        .iter()
        .filter(|t| {
            t.get("workflow").and_then(|w| w.get("name")).and_then(|n| n.as_str()) == Some("Managed Incident Response")
        })
        .filter_map(|t| t.get("workflow").and_then(|w| w.get("id")).and_then(|v| v.as_str()))
        .collect();
    assert_eq!(matched, vec!["WF_REAL"]);
}

#[tokio::test]
async fn shadow_scan_finds_multiple_matches() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/incident_workflows/triggers"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "triggers": [
                {"id": "T1", "workflow": {"id": "WF_A", "name": "Duplicate Name"}},
                {"id": "T2", "workflow": {"id": "WF_B", "name": "Duplicate Name"}}
            ],
            "more": false
        })))
        .mount(&server)
        .await;

    let client = mock_client(&server).await;
    let triggers = client
        .get_all_no_offset("/incident_workflows/triggers", "triggers")
        .await
        .unwrap();
    let mut ids: Vec<String> = triggers
        .iter()
        .filter(|t| t.get("workflow").and_then(|w| w.get("name")).and_then(|n| n.as_str()) == Some("Duplicate Name"))
        .filter_map(|t| {
            t.get("workflow")
                .and_then(|w| w.get("id"))
                .and_then(|v| v.as_str())
                .map(String::from)
        })
        .collect();
    ids.sort();
    ids.dedup();
    assert_eq!(ids, vec!["WF_A", "WF_B"]);
}

#[tokio::test]
async fn shadow_scan_finds_no_match() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/incident_workflows/triggers"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "triggers": [],
            "more": false
        })))
        .mount(&server)
        .await;

    let client = mock_client(&server).await;
    let triggers = client
        .get_all_no_offset("/incident_workflows/triggers", "triggers")
        .await
        .unwrap();
    assert!(triggers.is_empty());
}

// ---------------------------------------------------------------------------
// Import three-step flow: create workflow -> create trigger -> enable
// ---------------------------------------------------------------------------

#[tokio::test]
async fn import_three_step_flow() {
    let server = MockServer::start().await;

    // Step 1: query workflows by name - none found
    Mock::given(method("GET"))
        .and(path("/incident_workflows"))
        .and(query_param("query", "Auto-Manage P1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "incident_workflows": [],
            "limit": 25,
            "offset": 0,
            "more": false
        })))
        .expect(1)
        .mount(&server)
        .await;

    // Step 1: create workflow (disabled)
    Mock::given(method("POST"))
        .and(path("/incident_workflows"))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "incident_workflow": {
                "id": "WF_NEW",
                "name": "Auto-Manage P1",
                "is_enabled": false,
                "steps": [{
                    "id": "S1",
                    "name": "Set Incident Type",
                    "action_configuration": {
                        "action_id": "pagerduty.incident-management.update-incident-type",
                        "inputs": [{"name": "incident_type", "value": "Managed Incident"}]
                    }
                }]
            }
        })))
        .expect(1)
        .mount(&server)
        .await;

    // Step 2: list triggers to find existing
    Mock::given(method("GET"))
        .and(path("/incident_workflows/triggers"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "triggers": []
        })))
        .expect(1)
        .mount(&server)
        .await;

    // Step 2: create trigger
    Mock::given(method("POST"))
        .and(path("/incident_workflows/triggers"))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "trigger": {
                "id": "T_NEW",
                "trigger_type": "conditional",
                "condition": "incident.priority matches 'P1'",
                "workflow": {"id": "WF_NEW", "type": "workflow"}
            }
        })))
        .expect(1)
        .mount(&server)
        .await;

    // Verify: query by name
    let client = mock_client(&server).await;
    let resp = client.get("/incident_workflows?query=Auto-Manage P1").await.unwrap();
    assert!(resp["incident_workflows"].as_array().unwrap().is_empty());

    // Verify: create workflow
    let wf_body = json!({
        "incident_workflow": {
            "name": "Auto-Manage P1",
            "is_enabled": false,
            "steps": [{
                "name": "Set Incident Type",
                "action_configuration": {
                    "action_id": "pagerduty.incident-management.update-incident-type",
                    "inputs": [{"name": "incident_type", "value": "Managed Incident"}]
                }
            }]
        }
    });
    let wf_resp = client.post("/incident_workflows", wf_body).await.unwrap();
    let wf_id = wf_resp["incident_workflow"]["id"].as_str().unwrap();
    assert_eq!(wf_id, "WF_NEW");

    // Verify: check existing triggers
    let triggers = client.get("/incident_workflows/triggers").await.unwrap();
    assert!(triggers["triggers"].as_array().unwrap().is_empty());

    // Verify: create trigger referencing the workflow
    let trigger_body = json!({
        "trigger": {
            "trigger_type": "conditional",
            "condition": "incident.priority matches 'P1'",
            "workflow": {"id": wf_id, "type": "workflow"}
        }
    });
    let t_resp = client.post("/incident_workflows/triggers", trigger_body).await.unwrap();
    assert_eq!(t_resp["trigger"]["id"], "T_NEW");
    assert_eq!(t_resp["trigger"]["workflow"]["id"], "WF_NEW");
}
