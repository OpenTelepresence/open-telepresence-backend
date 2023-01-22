use anyhow::Result;
use webrtc::ice_transport::ice_connection_state::RTCIceConnectionState;
use webrtc::ice_transport::ice_gatherer_state::RTCIceGathererState;
use uuid::Uuid;
use crate::Group;
use serde::Serialize;
use tokio::{time::Duration, sync::Mutex};
use tokio::sync::mpsc;
use warp::ws::Message;
use webrtc::{
  api::{APIBuilder, media_engine::MediaEngine, interceptor_registry::register_default_interceptors},
  peer_connection::{
    configuration::RTCConfiguration,
    RTCPeerConnection, signaling_state::RTCSignalingState, peer_connection_state::RTCPeerConnectionState
  },
  ice_transport::{
    ice_candidate::RTCIceCandidate,
    ice_candidate::RTCIceCandidateInit,
    ice_server::RTCIceServer
  }, interceptor::registry::Registry, rtp_transceiver::{rtp_receiver::RTCRtpReceiver, rtp_codec::RTPCodecType}, track::{track_remote::TrackRemote, track_local::{track_local_static_rtp::TrackLocalStaticRTP, TrackLocalWriter}}, rtcp::payload_feedbacks::picture_loss_indication::PictureLossIndication
};
use webrtc::Error;

use std::collections::HashMap;
use std::sync::{Arc, Weak};
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;
use tracing::{debug, warn};

#[derive(Debug, Clone)]
pub struct WebRTCConnection {
  pub peer_connection: Arc<RTCPeerConnection>,
  sender: Arc<Box<mpsc::UnboundedSender<std::result::Result<Message, warp::Error>>>>,
  group: Arc<Mutex<Group>>,
  tracks: Arc<Mutex<HashMap<String, Arc<Track>>>>,
  id: Uuid

}

#[derive(Serialize)]
struct RTCSessionDescriptionInit {
  sdp: RTCSessionDescription
}

#[derive(Debug, Clone)]
pub struct Track {
  track: Arc<TrackLocalStaticRTP>,
  id: String
}

fn handle_track(remote_track: Option<Arc<TrackRemote>>, p2: &Weak<RTCPeerConnection>, 
                      group: &Arc<Mutex<Group>>, peer_identity: &String,
                      tracks: &Weak<Mutex<HashMap<String, Arc<Track>>>>) {
  let peer_identity2 = peer_identity.clone();
  if let Some(track) = remote_track {
    let media_ssrc = track.ssrc();
    // write rtcps on interval as there isn't a rtcp event
    let pc3 = p2.clone();
    tokio::spawn(async move {
      let mut result = Result::<usize, webrtc::Error>::Ok(0);
      while result.is_ok() {
        let timeout = tokio::time::sleep(Duration::from_secs(3));
        tokio::pin!(timeout);

        tokio::select! {
          _ = timeout.as_mut() =>{
            result = pc3.upgrade().unwrap().write_rtcp(&[Box::new(PictureLossIndication{
              sender_ssrc: 0,
              media_ssrc,
            })]).await.map_err(Into::into);
          }
        };
      }
    });
    // write rtps on event
    let track2 = track.clone();
    let group = group.clone();
    let tracks2 = tracks.clone();
    tokio::spawn(async move {
      let stream_id = peer_identity2.clone();
      let track_id = format!("{}_{}", stream_id, track2.kind().to_string());
      let local_track = Arc::new(TrackLocalStaticRTP::new(
          track2.codec().await.capability,
          track_id.clone(),
          stream_id, // FIXME: mabye this should be changed so the stream of
                     // video-audio is different for each pair
          ));
      debug!("Adding local track {:?} to group {:?}\n", local_track, group);
      let track = Track{track: local_track, id: track_id};
      let id = track.id.clone();
      let track = Arc::new(track.clone());
      if let Some(tracks3) = tracks2.upgrade() {
        tracks3.lock().await.insert(id, track.clone());
      }
      group.lock().await.notify_track(&track).await;
      while let Ok((rtp, _)) = track2.read_rtp().await {
        if let Err(err) = track.track.write_rtp(&rtp).await {
          if Error::ErrClosedPipe != err {
            warn!("output track write_rtp got error: {} and break", err);
            break;
          } else {
            warn!("output track write_rtp got error: {}", err);
          }
        }
      }
    });
    debug!("Got track {:?}", track);
  };
}

impl WebRTCConnection {
  pub async fn new(
    sender: Arc<Box<mpsc::UnboundedSender<std::result::Result<Message, warp::Error>>>>, 
    group: Arc<Mutex<Group>>) -> Result<Box<WebRTCConnection>, String> {
    let config = RTCConfiguration {
      ice_servers: vec![RTCIceServer {
        urls: vec!["stun:stun.l.google.com:19302".to_owned()],
        ..Default::default()
      }],
      ..Default::default()
    };

    let mut m = MediaEngine::default();
    // Setup the codecs you want to use.
    // We'll use a VP8 and Opus but you can also define your own
    m.register_default_codecs().unwrap();

    let mut registry = Registry::new();

    // Use the default set of Interceptors
    registry = register_default_interceptors(registry, &mut m).unwrap();

    // Create the API object with the MediaEngine
    let api = APIBuilder::new()
      .with_media_engine(m)
      .with_interceptor_registry(registry)
      .build();
    let peer_connection = api.new_peer_connection(config).await;
    let peer_connection = match peer_connection {
      Ok(conn) => conn,
      Err(error) => {
        return Err(format!("Unable to create new peer connection error:{}", error));
      }

    };
    let peer_connection = Arc::new(peer_connection);
    peer_connection
      .add_transceiver_from_kind(RTPCodecType::Audio, &[])
      .await.expect("Failed audio");
    peer_connection
      .add_transceiver_from_kind(RTPCodecType::Video, &[])
      .await.expect("Failed video");


    let res = Box::new(WebRTCConnection{
      peer_connection, 
      sender, 
      group: group.clone(), 
      tracks: Arc::new(Mutex::new(HashMap::new())),
      id: Uuid::new_v4()
    });
    Ok(res)
  }

  pub fn get_id(&self) -> String {
    return self.id.to_string();
  }

  pub async fn setup_callbacks(&self) {
    let ice_sender = self.sender.clone();

    let p2 = Arc::downgrade(&self.peer_connection);
    let group = self.group.clone();
    let peer_identity = self.get_id();
    let tracks = Arc::downgrade(&self.tracks);
    self.peer_connection.on_track(Box::new(move |remote_track: Option<Arc<TrackRemote>>, _rtp_receiver: Option<Arc<RTCRtpReceiver>>| {
      handle_track(remote_track, &p2, &group, &peer_identity, &tracks);
      Box::pin(async {})
    }));

    let webrtc_connection = self.clone();

    self.peer_connection.on_signaling_state_change(Box::new(move |state: RTCSignalingState| {
      // TODO: disconnect
      debug!("Signaling state change {}", state.to_string());
      Box::pin(async move {})
    }));

    self.peer_connection.on_negotiation_needed(Box::new(move|| {
      debug!("negotiation needed\n");
      let webrtc_connection2 = webrtc_connection.clone();
      tokio::spawn(async move {
        webrtc_connection2.renegotiate().await;
      });
      Box::pin(async {})
    }));

    self.peer_connection.on_ice_gathering_state_change(Box::new(move |s: RTCIceGathererState| {
      debug!("Peer ICE gathering state has changed: {:?}", s);
      Box::pin(async {})
    }));

    self.peer_connection.on_ice_connection_state_change(Box::new(move |s: RTCIceConnectionState| {
      debug!("Peer ICE connection state has changed: {:?}", s);
      Box::pin(async {})
    }));


    self.peer_connection.on_peer_connection_state_change(Box::new(move |s: RTCPeerConnectionState| {
      debug!("Peer Connection State has changed: {}", s);

      if s == RTCPeerConnectionState::Failed {
        // Wait until PeerConnection has had no network activity for 30 seconds or another failure. It may be reconnected using an ICE Restart.
        // Use webrtc.PeerConnectionStateDisconnected if you are interested in detecting faster timeout.
        // Note that the PeerConnection may come back from PeerConnectionStateDisconnected.
        debug!("Peer Connection has gone to failed exiting");
      }

      Box::pin(async {})
    }));

    self.peer_connection.on_ice_candidate(Box::new(move |candidate: Option<RTCIceCandidate>| {
      // forward candidate
      debug!("WEBRTC pre candidate");
      if let Some(candidate) = candidate {
        debug!("WEBRTC candidate {}", candidate.to_string());
        let ice_message = serde_json::to_string(&candidate).unwrap();
        let msg = warp::ws::Message::text(ice_message);
        debug!("WEBRTC candidate msg {:?}", msg);
        match ice_sender.as_ref().send(Ok(msg)) {
          Err(err) => warn!("Error sending ice candidate {:?}", err),
          _ => {}
        }
      }
      Box::pin(async {}) // we don't need to return anything, just send the candidate
    }));
  }

  pub async fn process_offer(&self, offer: String) {
    debug!("Offer before RTCSessionDescription is {:?}", offer);

    let description = match serde_json::from_str::<RTCSessionDescription>(&offer.as_str()) {
      Ok(value) => value,
      Err(err) => {
        warn!("Error creating session for offer {:?}", err);
        RTCSessionDescription::default()
      }
    };
    let res = self.peer_connection.set_remote_description(description).await;
    match res {
      Ok(_value) => debug!("Successfully added offer remote description"),
      Err(err) => warn!("Set description error {:?}", err),
    };
    // https://stackoverflow.com/questions/38036552/rtcpeerconnection-onicecandidate-not-fire
    let answer = match self.peer_connection.create_answer(None).await {
      Ok(a) => a,
      Err(err) => panic!("{}", err),
    };
    debug!("Answer is {:?}", answer);
    let local_res = self.peer_connection.set_local_description(answer.clone()).await;
    match local_res {
      Ok(value) => debug!("Set local description {:?}", value),
      Err(err) => warn!("Set local description error {:?}", err),
    };

    let answer = RTCSessionDescriptionInit{sdp: answer};
    let answer = serde_json::to_string(&answer).unwrap();
    let msg = warp::ws::Message::text(answer);
    match self.sender.send(Ok(msg)) {
      Err(err) => warn!("Error sending answer {:?}", err),
      _ => {}
    }
  }

  pub async fn process_answer(&self, answer: String) {
    let description = match serde_json::from_str::<RTCSessionDescription>(&answer.as_str()) {
      Ok(value) => value,
      Err(err) => {
        warn!("Error creating session description for answer {:?}", err);
        RTCSessionDescription::default()
      }
    };
    match self.peer_connection.set_remote_description(description).await {
      Ok(value) => debug!("Set remote description {:?}", value),
      Err(err) => warn!("Set remote description error {:?}", err),
    };
  }

  pub async fn process_ice_candidate(&self, candidate: String) {
    let candidate = match serde_json::from_str::<RTCIceCandidateInit>(candidate.as_str()) {
      Ok(value) => value,
      Err(err) => {
        warn!("Error adding ice_candidate {:?}", err);
        RTCIceCandidateInit::default()
      }
    };
    match self.peer_connection.add_ice_candidate(candidate).await {
      Ok(_value) => {
        debug!("Successfully added ice candidate");
      },
      Err(err) => {
        warn!("Error adding ice candidate {:?}", err);
      }
    }
  }

  pub async fn add_remote_track(&mut self, track: &Track) {
    match self.peer_connection.add_track(track.track.clone()).await {
      Ok(rtp_sender) => {
        debug!("Successfully added track\n");
        // Read incoming RTCP packets
        // Before these packets are returned they are processed by interceptors. For things
        // like NACK this needs to be called.
        tokio::spawn(async move {
          let mut rtcp_buf = vec![0u8; 1500];
          while let Ok((_, _)) = rtp_sender.read(&mut rtcp_buf).await { }
          debug!("End rtcp_buf {:?}\n", rtcp_buf);
          Result::<()>::Ok(())
        });
      },
      Err(err) => warn!("Unsuccessfully added track: {:?}\n", err)
    }
  }

  pub async fn renegotiate(&self) {
    debug!("Renegotiation started for {}", self.get_id());
    let offer = self.peer_connection.create_offer(None).await;
    if offer.is_err() {
      warn!("Error creating renegotiation offer {:?}", offer.err().unwrap());
      return;
    }

    let offer = offer.unwrap();
    let offer = RTCSessionDescriptionInit{sdp: offer};
    match self.peer_connection.set_local_description(offer.sdp.clone()).await {
      Ok(value) => {
        debug!("Set local description {:?}", value);
        let offer = serde_json::to_string(&offer).unwrap();
        let msg = warp::ws::Message::text(offer);
        match self.sender.send(Ok(msg)) {
          Err(err) => warn!("Error sending offer {:?}", err),
          _ => {}
        }
      },
      Err(err) => warn!("Set local description error {:?}", err),
    };

  }


  pub fn get_tracks(&self) -> &Arc<Mutex<HashMap<String, Arc<Track>>>> {
    &self.tracks
  }
}



