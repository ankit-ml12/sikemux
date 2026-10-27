import ExpoModulesCore

public class StashModule: Module {
  public func definition() -> ModuleDefinition {
    Name("SikemuxStash")

    Function("setShared") { (json: String?) in
      StashShared.write(json.map { Data($0.utf8) })
    }

    Function("setDeviceKey") { (hex: String?) in
      StashShared.setKey(hex)
    }
  }
}
