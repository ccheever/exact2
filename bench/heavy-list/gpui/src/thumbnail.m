#import <Foundation/Foundation.h>
#import <ImageIO/ImageIO.h>
#import <CoreGraphics/CoreGraphics.h>
#include <dlfcn.h>

void g1_boot_mark(const char *name) {
    typedef void (*Mark)(const char *);
    static Mark mark;
    static dispatch_once_t once;
    dispatch_once(&once, ^{ mark = (Mark)dlsym(RTLD_DEFAULT, "g1_startup_mark"); });
    if (mark) mark(name);
}

// ImageIO downsamples during decode, as the native benchmark does.
unsigned char *g1_thumbnail(const char *path, unsigned requestedWidth,
                            unsigned requestedHeight, unsigned *width, unsigned *height) {
    @autoreleasepool {
        NSURL *url = [NSURL fileURLWithPath:@(path)];
        CGImageSourceRef source = CGImageSourceCreateWithURL((__bridge CFURLRef)url,
            (__bridge CFDictionaryRef)@{(id)kCGImageSourceShouldCache: @NO});
        if (!source) return NULL;
        NSDictionary *properties = CFBridgingRelease(CGImageSourceCopyPropertiesAtIndex(source, 0, NULL));
        double w = [properties[(id)kCGImagePropertyPixelWidth] doubleValue];
        double h = [properties[(id)kCGImagePropertyPixelHeight] doubleValue];
        double scale = fmin(1, fmax(requestedWidth / w, requestedHeight / h));
        CGImageRef image = CGImageSourceCreateThumbnailAtIndex(source, 0, (__bridge CFDictionaryRef)@{
            (id)kCGImageSourceCreateThumbnailFromImageAlways: @YES,
            (id)kCGImageSourceCreateThumbnailWithTransform: @YES,
            (id)kCGImageSourceShouldCacheImmediately: @YES,
            (id)kCGImageSourceThumbnailMaxPixelSize: @(ceil(fmax(w, h) * scale))
        });
        CFRelease(source);
        if (!image) return NULL;
        *width = requestedWidth;
        *height = requestedHeight;
        unsigned char *bytes = calloc((size_t)*width * *height, 4);
        CGColorSpaceRef color = CGColorSpaceCreateWithName(kCGColorSpaceSRGB);
        CGContextRef context = CGBitmapContextCreate(bytes, *width, *height, 8, *width * 4,
            color, kCGImageAlphaPremultipliedFirst | kCGBitmapByteOrder32Little);
        double iw = CGImageGetWidth(image), ih = CGImageGetHeight(image);
        double fill = fmax(*width / iw, *height / ih);
        CGContextDrawImage(context, CGRectMake((*width - iw * fill) / 2,
            (*height - ih * fill) / 2, iw * fill, ih * fill), image);
        CGContextRelease(context);
        CGColorSpaceRelease(color);
        CGImageRelease(image);
        return bytes;
    }
}

// Optional linked probe; readiness describes the scene about to be submitted.
void g1_scene_complete(int complete, size_t item, float offset) {
    typedef void (*Note)(int, size_t, float);
    static Note note;
    static dispatch_once_t once;
    dispatch_once(&once, ^{ note = (Note)dlsym(RTLD_DEFAULT, "g1_ready"); });
    if (note) note(complete, item, offset);
}
