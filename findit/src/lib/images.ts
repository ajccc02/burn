"use client";
import { browserSupabase } from "./supabase/client";
import { supabaseConfig } from "./supabase/config";
import { IMAGE_BUCKET, MAX_IMAGE_BYTES } from "./constants";
export async function prepareImage(file: File): Promise<File> {
  if (!["image/jpeg", "image/png", "image/webp"].includes(file.type))
    throw new Error("Choose a JPEG, PNG, or WebP photograph.");
  if (!file.size || file.size > MAX_IMAGE_BYTES)
    throw new Error("Choose a photograph no larger than 5 MB.");
  const bytes = new Uint8Array(await file.slice(0, 12).arrayBuffer());
  const valid =
    file.type === "image/jpeg"
      ? bytes[0] === 255 && bytes[1] === 216 && bytes[2] === 255
      : file.type === "image/png"
        ? [137, 80, 78, 71, 13, 10, 26, 10].every(
            (value, index) => bytes[index] === value,
          )
        : String.fromCharCode(...bytes.slice(0, 4)) === "RIFF" &&
          String.fromCharCode(...bytes.slice(8, 12)) === "WEBP";
  if (!valid)
    throw new Error(
      "The file does not match its image type. Try a different photograph.",
    );
  const bitmap = await createImageBitmap(file).catch(() => {
    throw new Error(
      "This photograph could not be opened. Try a different image.",
    );
  });
  try {
    if (bitmap.width * bitmap.height > 40_000_000)
      throw new Error(
        "This image is too large to process. Choose a lower-resolution photograph.",
      );
    const scale = Math.min(1, 1600 / Math.max(bitmap.width, bitmap.height));
    const canvas = document.createElement("canvas");
    canvas.width = Math.max(1, Math.round(bitmap.width * scale));
    canvas.height = Math.max(1, Math.round(bitmap.height * scale));
    const context = canvas.getContext("2d");
    if (!context)
      throw new Error("Your browser could not process this photograph.");
    context.drawImage(bitmap, 0, 0, canvas.width, canvas.height);
    const blob = await new Promise<Blob>((resolve, reject) =>
      canvas.toBlob(
        (value) =>
          value
            ? resolve(value)
            : reject(
                new Error("Image processing failed. Try another photograph."),
              ),
        "image/webp",
        0.84,
      ),
    );
    if (blob.size > MAX_IMAGE_BYTES)
      throw new Error(
        "The processed photograph is still over 5 MB. Try a smaller image.",
      );
    return new File(
      [blob],
      blob.type === "image/webp" ? "photo.webp" : "photo.png",
      { type: blob.type },
    );
  } finally {
    bitmap.close();
  }
}
export async function uploadImage(
  file: File,
  userId: string,
  reportId: string,
  progress: (percent: number) => void,
): Promise<string> {
  const client = browserSupabase();
  const { data, error } = await client.auth.getSession();
  if (error || !data.session)
    throw new Error(
      "Your guest session expired. Refresh the page before uploading.",
    );
  const extension =
    file.type === "image/webp"
      ? "webp"
      : file.type === "image/jpeg"
        ? "jpg"
        : "png";
  const path = `${userId}/${reportId}/${crypto.randomUUID()}.${extension}`;
  const { url, key } = supabaseConfig();
  await new Promise<void>((resolve, reject) => {
    const request = new XMLHttpRequest();
    request.open("POST", `${url}/storage/v1/object/${IMAGE_BUCKET}/${path}`);
    request.setRequestHeader(
      "Authorization",
      `Bearer ${data.session!.access_token}`,
    );
    request.setRequestHeader("apikey", key);
    request.setRequestHeader("Content-Type", file.type);
    request.setRequestHeader("x-upsert", "false");
    request.timeout = 60000;
    request.upload.onprogress = (event) => {
      if (event.lengthComputable)
        progress(Math.round((event.loaded / event.total) * 100));
    };
    request.onload = () =>
      request.status >= 200 && request.status < 300
        ? resolve()
        : reject(
            new Error(
              request.status === 413
                ? "The photograph exceeds the storage limit. Choose a smaller image."
                : "Photograph upload failed. Check your connection and storage setup, then try again.",
            ),
          );
    request.onerror = () =>
      reject(
        new Error(
          "The photograph could not be uploaded. Check your internet connection and try again.",
        ),
      );
    request.ontimeout = () =>
      reject(new Error("The upload timed out. Please try again."));
    request.send(file);
  });
  return path;
}
