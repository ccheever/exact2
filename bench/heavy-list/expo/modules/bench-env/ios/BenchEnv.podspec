Pod::Spec.new do |s|
  s.name           = 'BenchEnv'
  s.version        = '1.0.0'
  s.summary        = 'Launch environment constants for the heavy list benchmark'
  s.author         = ''
  s.homepage       = 'https://example.invalid'
  s.license        = 'MIT'
  s.platforms      = { :ios => '15.1' }
  s.source         = { git: '' }
  s.static_framework = true
  s.dependency 'ExpoModulesCore'
  s.source_files = '**/*.{h,m,swift}'
end
