// Service worker entry point for the "Add to ggg" extension.
//
// Responsibilities:
//   1. Register context menu items on install/startup.
//   2. Forward menu clicks to ggg-bridge via Native Messaging.
//   3. Surface success/error feedback through chrome.notifications.
//
// The native host name must match the manifest installed on disk and
// registered under HKCU\Software\Google\Chrome\NativeMessagingHosts.

const NATIVE_HOST = "com.ggg.bridge";

const MENU_ITEMS = [
  { id: "ggg-add-link",      title: "Add link to ggg",      contexts: ["link"] },
  { id: "ggg-add-page",      title: "Add this page to ggg", contexts: ["page"] },
  { id: "ggg-add-image",     title: "Add image to ggg",     contexts: ["image"] },
  { id: "ggg-add-video",     title: "Add video to ggg",     contexts: ["video"] },
  { id: "ggg-add-audio",     title: "Add audio to ggg",     contexts: ["audio"] },
  { id: "ggg-add-selection", title: "Add selection as URL", contexts: ["selection"] },
];

chrome.runtime.onInstalled.addListener(() => {
  // Wipe and recreate to keep the menu in sync after upgrades.
  chrome.contextMenus.removeAll(() => {
    for (const item of MENU_ITEMS) {
      chrome.contextMenus.create(item);
    }
  });
});

chrome.contextMenus.onClicked.addListener((info, _tab) => {
  const url = pickUrl(info);
  if (!url) {
    notify("ggg", "No URL found in this context.");
    return;
  }
  sendUrl(url);
});

/**
 * Resolve the URL to add based on which context menu item was clicked.
 * Order matters: a click on a link inside a page should prefer the link,
 * a click on an image should prefer the image, etc.
 */
function pickUrl(info) {
  switch (info.menuItemId) {
    case "ggg-add-link":      return info.linkUrl;
    case "ggg-add-image":     return info.srcUrl || info.linkUrl;
    case "ggg-add-video":     return info.srcUrl || info.linkUrl;
    case "ggg-add-audio":     return info.srcUrl || info.linkUrl;
    case "ggg-add-page":      return info.pageUrl;
    case "ggg-add-selection": {
      const text = (info.selectionText || "").trim();
      // Only accept selections that parse as a URL — otherwise the user
      // probably meant to add the page rather than highlighted prose.
      try {
        return new URL(text).toString();
      } catch {
        return null;
      }
    }
    default: return info.linkUrl || info.srcUrl || info.pageUrl;
  }
}

/**
 * Send `add_url` to the native host and toast the result.
 *
 * Uses `sendNativeMessage` (one-shot) rather than `connectNative` so the
 * host process exits between requests. This keeps the bridge stateless
 * and avoids leaving zombie processes if the service worker is killed.
 */
function sendUrl(url) {
  chrome.runtime.sendNativeMessage(NATIVE_HOST, { type: "add_url", url }, (response) => {
    if (chrome.runtime.lastError) {
      notify("ggg: bridge unreachable", chrome.runtime.lastError.message || "Unknown error");
      return;
    }
    if (!response) {
      notify("ggg: empty response", "The bridge returned no message.");
      return;
    }
    if (response.type === "ok") {
      //notify("ggg: queued", response.message || url);
    } else if (response.type === "error") {
      notify("ggg: error", response.message || "Unknown error");
    } else {
      notify("ggg", `Unexpected response: ${response.type}`);
    }
  });
}

function notify(title, message) {
  // iconUrl is required by Chrome — fall back to a tiny inline PNG so the
  // extension does not need a separate icon asset on disk.
  chrome.notifications.create({
    type: "basic",
    iconUrl: TRANSPARENT_ICON,
    title,
    message: String(message).slice(0, 500),
  });
}

// 1x1 transparent PNG as a data URL — used as the notification icon.
const TRANSPARENT_ICON =
  "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNkYAAAAAYAAjCB0C8AAAAASUVORK5CYII=";

