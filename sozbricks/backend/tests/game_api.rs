use std::{collections::HashMap, time::Duration};

use axum::{Router, Json, body::{Body,to_bytes}, http::{HeaderMap,HeaderValue,Request,StatusCode,header}, routing::get};
use serde_json::{Value,json};
use sozbricks_api::{app_with_database, config::{Config,AuthConfig}, database::Database};
use sqlx::{Row, postgres::{PgPoolOptions,PgConnectOptions,PgSslMode}};
use tower::ServiceExt;
use uuid::Uuid;

async fn request(router: &Router, method: &str, path: &str, token: &str, body: Option<Value>) -> (StatusCode,Value) {
    let mut request = Request::builder().method(method).uri(path).header(header::AUTHORIZATION,format!("Bearer {token}"));
    let payload = if let Some(body) = body { request = request.header(header::CONTENT_TYPE,"application/json"); body.to_string() } else { String::new() };
    let response = router.clone().oneshot(request.body(Body::from(payload)).unwrap()).await.unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(),1048576).await.unwrap();
    let body = serde_json::from_slice(&bytes).unwrap_or_else(|_|panic!("Non-JSON response: {}",String::from_utf8_lossy(&bytes)));
    (status,body)
}

async fn start(router: &Router, token: &str, level: i16) -> Value {
    let (status,result) = request(router,"POST","/v1/game-sessions",token,Some(json!({"level_id":level,"request_id":Uuid::new_v4()}))).await;
    assert_eq!(status,StatusCode::OK,"{result}");
    result
}

fn result(won: bool, score: i64, word: &str) -> Value { json!({"won":won,"score":score,"collected_word":word,"duration_seconds":35}) }

#[tokio::test]
#[ignore = "requires disposable PostgreSQL on the cloud CI runner"]
async fn account_progress_import_results_and_runtime_isolation() {
    assert_eq!(std::env::var("CI").as_deref(),Ok("true"));
    let url = std::env::var("TEST_DATABASE_URL").unwrap();
    let mut parsed: reqwest::Url = url.parse().unwrap();
    assert!(matches!(parsed.host_str(),Some("127.0.0.1"|"localhost")));
    let admin_options: PgConnectOptions = url.parse().unwrap();
    let admin = PgPoolOptions::new().max_connections(1).connect_with(admin_options.ssl_mode(PgSslMode::Disable)).await.unwrap();
    let name = format!("sozbricks_{}",Uuid::new_v4().simple());
    sqlx::query(&format!("CREATE DATABASE {name}")).execute(&admin).await.unwrap();
    parsed.set_path(&format!("/{name}"));
    let mut config = Config::from_values(&HashMap::from([
        ("DATABASE_URL".into(),parsed.to_string()),("DATABASE_SSL_MODE".into(),"disable".into())
    ])).unwrap();
    let database = Database::new(config.database.as_ref());
    let pool = database.pool().unwrap();
    sqlx::raw_sql("CREATE SCHEMA auth; CREATE TABLE auth.users(id uuid PRIMARY KEY);").execute(pool).await.unwrap();
    database.prepare().await.unwrap();
    let users = [Uuid::new_v4(),Uuid::new_v4(),Uuid::new_v4()];
    for id in &users[..2] { sqlx::query("INSERT INTO auth.users(id) VALUES($1)").bind(id).execute(pool).await.unwrap(); }
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let auth = Router::new().route("/auth/v1/user",get(move |headers:HeaderMap|async move {
        let index = match headers.get(header::AUTHORIZATION).and_then(|value|value.to_str().ok()) {
            Some("Bearer alpha") => 0, Some("Bearer beta") => 1, Some("Bearer missing") => 2,
            _ => return (StatusCode::UNAUTHORIZED,Json(json!({}))),
        };
        (StatusCode::OK,Json(json!({"id":users[index],"role":"authenticated","email":"player@example.com","app_metadata":{"providers":["google"]},"user_metadata":{"full_name":"Игрок"}})))
    }));
    let worker = tokio::spawn(async move { axum::serve(listener,auth).await.unwrap(); });
    config.auth = Some(AuthConfig { user_url:format!("http://{address}/auth/v1/user").parse().unwrap(),publishable_key:HeaderValue::from_static("sb_publishable_test"),timeout:Duration::from_secs(2) });
    let router = app_with_database(config,database.clone());

    let (status,identity) = request(&router,"GET","/v1/auth/me","alpha",None).await;
    assert_eq!(status,StatusCode::OK,"{identity}");
    let count:i64 = sqlx::query_scalar("SELECT count(*) FROM sozbricks.profiles WHERE user_id=$1").bind(users[0]).fetch_one(pool).await.unwrap();
    assert_eq!(count,1);
    let (status,error) = request(&router,"GET","/v1/auth/me","missing",None).await;
    assert_eq!(status,StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(error["error"]["code"],"auth_database_mismatch");

    let (status,profile) = request(&router,"PATCH","/v1/profile","alpha",Some(json!({"display_name":"  Айдос  ","hero":"aiaru"}))).await;
    assert_eq!(status,StatusCode::OK,"{profile}");
    assert_eq!(profile["profile"]["display_name"],"Айдос");
    request(&router,"GET","/v1/auth/me","alpha",None).await;
    let (_,profile) = request(&router,"GET","/v1/profile","alpha",None).await;
    assert_eq!(profile["profile"]["display_name"],"Айдос");
    let (status,_) = request(&router,"PATCH","/v1/profile","alpha",Some(json!({"hero":"unknown"}))).await;
    assert_eq!(status,StatusCode::UNPROCESSABLE_ENTITY);
    let (status,_) = request(&router,"POST","/v1/game-sessions","alpha",Some(json!({"level_id":2,"request_id":Uuid::new_v4()}))).await;
    assert_eq!(status,StatusCode::FORBIDDEN);

    let imported = json!({"import_id":Uuid::new_v4(),"save":{"version":1,"hero":"aiaru","completed":[1],"best":{"1":1200},"history":[{"id":"guest-1","level":1,"word":"АТ","score":1200,"won":true,"seconds":50,"at":1700000000000_i64}],"dark":true,"sound":true,"vibration":true,"demoLamports":100000000}});
    let (status,import) = request(&router,"POST","/v1/progress/import","alpha",Some(imported.clone())).await;
    assert_eq!(status,StatusCode::OK,"{import}");
    assert_eq!(import["awarded_lamports"],0);
    assert_eq!(import["progress"]["completed"],json!([1]));
    assert_eq!(import["progress"]["earned_lamports"],0);
    let (status,import) = request(&router,"POST","/v1/progress/import","alpha",Some(imported.clone())).await;
    assert_eq!(status,StatusCode::OK); assert_eq!(import["repeated"],true);
    let mut changed = imported.clone(); changed["save"]["completed"] = json!([1,2]);
    assert_eq!(request(&router,"POST","/v1/progress/import","alpha",Some(changed)).await.0,StatusCode::CONFLICT);
    let (_,other) = request(&router,"GET","/v1/progress","beta",None).await;
    assert_eq!(other["completed"],json!([]));
    assert_eq!(other["earned_lamports"],0);

    let attempt = start(&router,"alpha",2).await;
    let id = attempt["id"].as_str().unwrap();
    let request_id = attempt["request_id"].clone();
    let (_,same) = request(&router,"POST","/v1/game-sessions","alpha",Some(json!({"level_id":2,"request_id":request_id}))).await;
    assert_eq!(same["id"],attempt["id"]);
    let path = format!("/v1/game-sessions/{id}/finish");
    assert_eq!(request(&router,"POST",&path,"beta",Some(result(true,1000,"АНА"))).await.0,StatusCode::NOT_FOUND);
    assert_eq!(request(&router,"POST",&path,"alpha",Some(result(true,-1,"АНА"))).await.0,StatusCode::UNPROCESSABLE_ENTITY);
    let submitted = result(true,1130,"АНА");
    let (first,second) = tokio::join!(request(&router,"POST",&path,"alpha",Some(submitted.clone())),request(&router,"POST",&path,"alpha",Some(submitted.clone())));
    assert_eq!(first.0,StatusCode::OK,"{}",first.1);
    assert_eq!(second.0,StatusCode::OK,"{}",second.1);
    assert_ne!(first.1["repeated"],second.1["repeated"]);
    assert_eq!(first.1["score"],1130);
    assert_eq!(first.1["awarded_lamports"],100000000);
    assert_eq!(first.1["progress"]["completed"],json!([1,2]));
    assert_eq!(request(&router,"POST",&path,"alpha",Some(result(true,9999,"АНА"))).await.0,StatusCode::CONFLICT);
    let count:i64 = sqlx::query_scalar("SELECT count(*) FROM sozbricks.reward_ledger WHERE user_id=$1 AND level_id=2").bind(users[0]).fetch_one(pool).await.unwrap();
    assert_eq!(count,1);
    let again = start(&router,"alpha",2).await;
    let path = format!("/v1/game-sessions/{}/finish",again["id"].as_str().unwrap());
    let (status,again) = request(&router,"POST",&path,"alpha",Some(result(true,2000,"АНА"))).await;
    assert_eq!(status,StatusCode::OK,"{again}");
    assert_eq!(again["awarded_lamports"],0);
    assert_eq!(again["progress"]["best"]["2"],2000);

    let lost = start(&router,"alpha",3).await;
    let path = format!("/v1/game-sessions/{}/finish",lost["id"].as_str().unwrap());
    let (status,lost) = request(&router,"POST",&path,"alpha",Some(result(false,3000,"К"))).await;
    assert_eq!(status,StatusCode::OK,"{lost}");
    assert_eq!(lost["awarded_lamports"],0);
    assert_eq!(lost["progress"]["completed"],json!([1,2]));
    assert_eq!(lost["progress"]["best"]["3"],3000);

    let stale = start(&router,"alpha",3).await;
    let fresh = start(&router,"alpha",3).await;
    let stale_path = format!("/v1/game-sessions/{}/finish",stale["id"].as_str().unwrap());
    assert_eq!(request(&router,"POST",&stale_path,"alpha",Some(result(false,0,""))).await.0,StatusCode::CONFLICT);
    let fresh_id:Uuid = fresh["id"].as_str().unwrap().parse().unwrap();
    sqlx::query("UPDATE sozbricks.game_sessions SET started_at=now()-interval '3 hours',expires_at=now()-interval '1 hour' WHERE id=$1").bind(fresh_id).execute(pool).await.unwrap();
    let path = format!("/v1/game-sessions/{fresh_id}/finish");
    assert_eq!(request(&router,"POST",&path,"alpha",Some(result(false,0,""))).await.0,StatusCode::GONE);
    let abandon = start(&router,"alpha",3).await;
    let path = format!("/v1/game-sessions/{}/abandon",abandon["id"].as_str().unwrap());
    assert_eq!(request(&router,"POST",&path,"alpha",None).await.0,StatusCode::OK);
    assert_eq!(request(&router,"POST",&path,"alpha",None).await.0,StatusCode::OK);

    let (_,history) = request(&router,"GET","/v1/history?limit=2&offset=0","alpha",None).await;
    assert_eq!(history["items"].as_array().unwrap().len(),2);
    assert_eq!(history["imported_history"].as_array().unwrap().len(),1);
    assert_eq!(history["items"][0]["source"],"frontend_reported");
    let (_,rewards) = request(&router,"GET","/v1/rewards","alpha",None).await;
    assert_eq!(rewards["items"].as_array().unwrap().len(),1);
    assert_eq!(rewards["total_lamports"],100000000);
    assert_eq!(rewards["solana_payments_enabled"],false);
    let (_,other) = request(&router,"GET","/v1/history","beta",None).await;
    assert_eq!(other["items"],json!([]));
    assert_eq!(request(&router,"GET","/v1/history?limit=101","alpha",None).await.0,StatusCode::UNPROCESSABLE_ENTITY);

    let runtime = database.runtime_pool().unwrap();
    let role:String = sqlx::query_scalar("SELECT current_user::text").fetch_one(runtime).await.unwrap();
    assert_eq!(role,"sozbricks_runtime");
    let invisible:i64 = sqlx::query_scalar("SELECT count(*) FROM sozbricks.profiles").fetch_one(runtime).await.unwrap();
    assert_eq!(invisible,0);
    let forbidden = sqlx::query("SELECT * FROM auth.users").execute(runtime).await.unwrap_err();
    assert_eq!(forbidden.as_database_error().unwrap().code().unwrap(),"42501");
    let row = sqlx::query("SELECT count(*) AS count FROM sozbricks.game_results WHERE user_id=$1").bind(users[0]).fetch_one(pool).await.unwrap();
    assert_eq!(row.get::<i64,_>("count"),3);
    worker.abort();
    let _ = worker.await;
    database.close().await;
    sqlx::query(&format!("DROP DATABASE {name}")).execute(&admin).await.unwrap();
    admin.close().await;
}
