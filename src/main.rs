use futures::{stream::Stream, StreamExt};
use r2r::scene_manipulation_msgs::srv::{LookupTransform, ManipulateScene};
use r2r::std_msgs::msg::Bool;
use r2r::std_srvs::srv::Trigger;
use r2r::{QosProfile, ServiceRequest};
use std::sync::{Arc, Mutex};

pub static PARENT_ID: &'static str = "floor";
pub static FRAME_ID: &'static str = "aruco_5";
pub static LOCKED_FRAME_ID: &'static str = "locked_aruco";

struct State {
    frame_exists: bool,
    frame_locked: bool,
}

async fn run_service(lookup_client: r2r::Client<LookupTransform::Service>,
                     exists_pub: r2r::Publisher<Bool>,
                     locked_pub: r2r::Publisher<Bool>,
                     shared_state: Arc<Mutex<State>>) {
    loop {
        // poll state of the frame
        let msg = LookupTransform::Request {
            parent_frame_id: PARENT_ID.to_string(),
            child_frame_id: FRAME_ID.to_string(),
        };
        let response = lookup_client.request(&msg).expect("could not make request");

        if let Ok(response) = response.await {
            shared_state.lock().unwrap().frame_exists = response.success;
        }

        // publish our state
        let (exists, locked) =
        {
            let shared_state = shared_state.lock().unwrap();
            (shared_state.frame_exists, shared_state.frame_locked)
        };
        exists_pub.publish(&Bool { data: exists }).expect("could not publish");
        locked_pub.publish(&Bool { data: locked }).expect("could not publish");

        // sleep before next poll
        tokio::time::sleep(tokio::time::Duration::from_millis(250)).await;
    }
}

async fn run_lock_service(mut service: impl Stream<Item = ServiceRequest<Trigger::Service>> + Unpin,
                          sms_client: Arc<Mutex<r2r::Client<ManipulateScene::Service>>>,
                          shared_state: Arc<Mutex<State>>) {
    loop {
        if let Some(req) = service.next().await {
            if !shared_state.lock().unwrap().frame_exists {
                println!("got lock request but have not seen a frame");
                req.respond(Trigger::Response::default()).expect("could not send response");
                continue;
            }
            println!("got lock request.");
            let response = {
                let command = format!("clone:{}", FRAME_ID);
                let msg = ManipulateScene::Request {
                    child_frame_id: LOCKED_FRAME_ID.to_string(),
                    parent_frame_id: PARENT_ID.to_string(),
                    command,
                    same_position_in_world: true,
                    .. ManipulateScene::Request::default()
                };
                println!("lock request {:?}", msg);
                sms_client.lock().unwrap().request(&msg).expect("could not make request")
            };
            if let Ok(response) = response.await {
                if response.success {
                    shared_state.lock().unwrap().frame_locked = true;
                } else {
                    println!("sms error: {}", response.info);
                }
            } else {
                println!("error making request to sms");
            }
            req.respond(Trigger::Response::default()).expect("could not send response");
        }
    }
}

async fn run_clear_service(mut service: impl Stream<Item = ServiceRequest<Trigger::Service>> + Unpin,
                           sms_client: Arc<Mutex<r2r::Client<ManipulateScene::Service>>>,
                           shared_state: Arc<Mutex<State>>) {
    loop {
        if let Some(req) = service.next().await {
            if !shared_state.lock().unwrap().frame_locked {
                println!("got clear request but have no locked frames");
                req.respond(Trigger::Response::default()).expect("could not send response");
                continue;
            }
            println!("got clear request.");
            let response = {
                let msg = ManipulateScene::Request {
                    child_frame_id: LOCKED_FRAME_ID.to_string(),
                    command: "remove".to_string(),
                    .. ManipulateScene::Request::default()
                };
                sms_client.lock().unwrap().request(&msg).expect("could not make request")
            };
            if let Ok(response) = response.await {
                if response.success {
                    shared_state.lock().unwrap().frame_locked = false;
                } else {
                    println!("sms error: {}", response.info);
                }
            } else {
                println!("error making request to sms");
            }
            req.respond(Trigger::Response::default()).expect("could not send response");
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // setup the node
    let ctx = r2r::Context::create()?;
    let mut node = r2r::Node::create(ctx, "frame_locker", "")?;

    let shared_state = Arc::new(Mutex::new(State {
        frame_locked: false,
        frame_exists: false,
    }));

    let (sms_client, ready_sms) = {
        let c = node.create_client::<ManipulateScene::Service>("/manipulate_scene")?;

        let ready = node.is_available(&c).unwrap();
        (Arc::new(Mutex::new(c)), ready)
    };

    let (lookup_client, ready_lookup) = {
        let c = node.create_client::<LookupTransform::Service>("/lookup_transform")?;

        let ready = node.is_available(&c).unwrap();
        (c, ready)
    };

    let lock_service = node.create_service::<Trigger::Service>("lock_frames")?;
    let clear_service = node.create_service::<Trigger::Service>("clear_locked_frames")?;

    let exists_pub = node.create_publisher::<Bool>("frame_exists", QosProfile::default())?;
    let locked_pub = node.create_publisher::<Bool>("frame_locked", QosProfile::default())?;

    // keep the node alive
    let handle = std::thread::spawn(move || loop {
        node.spin_once(std::time::Duration::from_millis(1000));
    });

    println!("waiting for sms service...");
    ready_sms.await.expect("failed to complete waiting");
    println!("service available.");

    println!("waiting for lookup service...");
    ready_lookup.await.expect("failed to complete waiting");
    println!("service available.");

    tokio::task::spawn(run_lock_service(lock_service, sms_client.clone(), shared_state.clone()));
    tokio::task::spawn(run_clear_service(clear_service, sms_client.clone(), shared_state.clone()));

    tokio::task::spawn(run_service(lookup_client,
                                   exists_pub,
                                   locked_pub,
                                   shared_state)).await?;

    handle.join().expect("could not join spinner thread");

    Ok(())
}
