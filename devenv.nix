{
  pkgs,
  config,
  ...
}: {
  env = {
    AWS_ACCESS_KEY_ID = config.services.minio.accessKey;
    AWS_SECRET_ACCESS_KEY = config.services.minio.secretKey;
    AWS_REGION = "us-east-1";
  };

  services.minio = {
    enable = true;

    accessKey = "testAccessKey";
    secretKey = "testSecretKey";
  };
}
