import AppKit
import XCTest

// Lane r4-timeline: an image chip's accent (T3ImageAccent.swift) against the reference's
// averageImageColor for the same picture (Chrome measured rgb(129 140 168) for photo.png).
final class ImageAccentTests: XCTestCase {
    func testAverageMatchesTheReferenceCanvasSample() throws {
        // photo.png: a blue card with a yellow square and a red circle (the lanes' attachment fixture).
        let data = try XCTUnwrap(Data(base64Encoded: "iVBORw0KGgoAAAANSUhEUgAAAUAAAADICAIAAAAWZq/8AAAD2UlEQVR4nO3cwW1TQRRA0YCozSuKYD1FsEgRWVMEq9RCMSyMLIEQRIg/M/f/c9ZJPO/LV29kS3l3e/72BDS9X30A4N8JGMIEDGEChjABQ5iAIUzAECZgCBMwhAkYwgQMYQKGMAFDmIAhTMAQJmAIEzCECRjCBAxhAoYwAUOYgCFMwBAmYAgTMIQJGMIEDGEChjABQ5iAIUzAECZgCBMwhAkYwgQMYQKGMAFDmIAhTMAQ9mHVC3+9fVr10rzdx9cvq4/An9jAECZgCBMwhAkYwgQMYQKGMAFDmIAhTMAQJmAIEzCECRjCBAxhAoYwAUOYgCFMwBAmYAgTMIQt+59Y8N+9vH5+y4+N2/PTWQiY8xf7599K9yxgrtLtX/9asWQBc9Fuz1GygAk4Ot3fvlwiYwGztcnp5jIWMJtamG4oY98Ds6NN6t32PA82MHvZNpWXLVexDcxGtq132xMKmF3s1kbinK7QrLdVEq3rtA3MYrl6tzq5gLl6A+nzC5jrvvtPMIWAWeMc9S6fRcAQJmAWONP6XTuRgJntfPUunEvATHXWeldNJ2AIEzDznHv9LplRwExyhXrnTypgCBMwM1xn/U6eV8AQJmAOd7X1O3NqAUOYgDnWNdfvtNkFDGEChjABc6Ar35/nPAEBQ5iAIUzAHMX9ecJzEDCECRjCBAxhAoYwAXMIn2DNeRoChjABQ5iAIUzAECZgCBMwhAkYwgQMYQKGMAFDmIAhTMAQJmAOMW7Pq49wiachYAgTMIQJGMIEDGEC5ig+x5rwHAQMYQKGMAFzILfocfATEDCECRjCBMyxrnyLHsfPLmAIEzCHu+YSHlOmFjCECZgZrraEx6x5BQxhAmaS6yzhMXFSATPPFRoec2cUMIQJmKnOvYTH9OkEzGxnbXismEvALHC+hseiiQQMYQJmjTMt4bFuFgGzzDkaHkunEDAr1Rseq88vYK7eQPrkH1YfAH6U8PL6+SlibJDunQ3MLvapInROAbORrdpInNAVmr1se50em6V7ZwOzo91qGZud58EGZlObrOKxa7p3AmZrCzMee6d7J2ACJmc8CuneCZiMR1cHlTw63T4ImKuXPILdPgiYsPFze2/sOV3sL97dnr+tPgPwj3wPDGEChjABQ5iAIUzAECZgCBMwhAkYwgQMYQKGMAFDmIAhTMAQJmAIEzCECRjCBAxhAoYwAUOYgCFMwBAmYAgTMIQJGMIEDGEChjABQ5iAIUzAECZgCBMwhAkYwgQMYQKGMAFDmIAhTMAQJmAIEzCECRjCBAxhAoYwAUOYgCFMwBAmYAgTMIQJGJ66vgMQpb77qSizrgAAAABJRU5ErkJggg=="))
        let channels = (T3ImageAccent.average(data) ?? "").split(separator: " ").compactMap { Int($0) }
        XCTAssertEqual(channels.count, 3)
        for (value, want) in zip(channels, [129, 140, 168]) { XCTAssertLessThanOrEqual(abs(value - want), 3, "\(channels)") }
        XCTAssertNil(T3ImageAccent.average(Data("not an image".utf8)))
    }
}
