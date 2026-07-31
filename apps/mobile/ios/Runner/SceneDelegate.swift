import Flutter
import UIKit

class SceneDelegate: FlutterSceneDelegate {
  private static let privacyOverlayTag = 0x4D_55_58

  override func sceneWillResignActive(_ scene: UIScene) {
    super.sceneWillResignActive(scene)
    guard let windowScene = scene as? UIWindowScene else {
      return
    }
    for window in windowScene.windows where window.viewWithTag(Self.privacyOverlayTag) == nil {
      let overlay = UIView(frame: window.bounds)
      overlay.tag = Self.privacyOverlayTag
      overlay.autoresizingMask = [.flexibleWidth, .flexibleHeight]
      overlay.backgroundColor = .systemBackground

      let title = UILabel()
      title.translatesAutoresizingMaskIntoConstraints = false
      title.text = "Muxport"
      title.font = .preferredFont(forTextStyle: .title1)
      title.textColor = .label
      overlay.addSubview(title)
      NSLayoutConstraint.activate([
        title.centerXAnchor.constraint(equalTo: overlay.centerXAnchor),
        title.centerYAnchor.constraint(equalTo: overlay.centerYAnchor),
      ])
      window.addSubview(overlay)
    }
  }

  override func sceneDidBecomeActive(_ scene: UIScene) {
    super.sceneDidBecomeActive(scene)
    guard let windowScene = scene as? UIWindowScene else {
      return
    }
    for window in windowScene.windows {
      window.viewWithTag(Self.privacyOverlayTag)?.removeFromSuperview()
    }
  }
}
