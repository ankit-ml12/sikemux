Pod::Spec.new do |s|
  s.name           = 'SikemuxStash'
  s.version        = '0.1.0'
  s.summary        = 'What the share extension reads from the app: its chats, the accounts server and the phone key.'
  s.license        = 'UNLICENSED'
  s.author         = 'Sikemux'
  s.homepage       = 'https://sikemux.com'
  s.platforms      = { :ios => '16.4' }
  s.source         = { git: '' }
  s.static_framework = true

  s.dependency 'ExpoModulesCore'

  s.source_files = 'Module/**/*.swift', 'Shared/**/*.swift'
end
